#![allow(dead_code)]

use crate::gateway::converter::build_kiro_payload;
use crate::gateway::eventstream::decode_message;
use crate::gateway::models::NormalizedMessage;
use crate::gateway::response_cache::ResponseCache;
use crate::clients::http_client::build_q_service_url;
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 压缩对话历史
///
/// 策略：
/// 1. 保留最后 2 轮对话（最近的上下文）
/// 2. 将中间的历史对话发送给 LLM 生成摘要（支持三层缓存）
/// 3. 用摘要替换中间的历史消息
/// 4. 返回：[系统消息] + [摘要] + [最近2轮对话]
pub async fn compress_conversation_history(
    http: &reqwest::Client,
    access_token: &str,
    region: &str,
    messages: &mut Vec<NormalizedMessage>,
    model_id: &str,
    _max_input_tokens: usize,
    mut cache: Option<&mut ResponseCache>,
    session_id: Option<&str>,
) -> Result<bool, String> {
    // 至少需要 5 条消息才值得压缩
    if messages.len() < 5 {
        log::info!("[压缩] 消息数量不足 5 条，跳过压缩");
        return Ok(false);
    }

    log::info!("[压缩] 开始压缩对话历史，当前消息数: {}", messages.len());

    // 分离消息：系统消息 + 需要压缩的消息 + 最近的消息
    let mut system_messages = Vec::new();

    // 找出系统消息
    for msg in messages.iter() {
        if msg.role == "system" {
            system_messages.push(msg.clone());
        }
    }

    // 找出非系统消息
    let non_system: Vec<_> = messages.iter().filter(|m| m.role != "system").cloned().collect();

    if non_system.len() < 5 {
        log::info!("[压缩] 非系统消息不足 5 条，跳过压缩");
        return Ok(false);
    }

    // 保留最后 4 条消息（2轮对话）
    let preserve_count = 4.min(non_system.len());
    let compress_count = non_system.len() - preserve_count;

    let to_compress = non_system[..compress_count].to_vec();
    let recent_messages = non_system[compress_count..].to_vec();

    log::info!(
        "[压缩] 系统消息: {}, 待压缩: {}, 保留: {}",
        system_messages.len(),
        to_compress.len(),
        recent_messages.len()
    );

    // 计算待压缩消息的哈希值（用于缓存键）
    let messages_hash = calculate_messages_hash(&to_compress);
    let message_count = to_compress.len();
    let total_chars: usize = to_compress.iter()
        .filter_map(|m| m.content.as_ref())
        .map(|c| c.to_string().len())
        .sum();

    // 尝试从缓存获取摘要
    let summary = if let (Some(cache_ref), Some(sid)) = (cache.as_mut(), session_id) {
        if let Some(cached_entry) = cache_ref.get(sid, &messages_hash, message_count, total_chars) {
            log::info!("[压缩] 命中缓存！使用缓存的摘要（节省 {} 输入 tokens，{} 输出 tokens）",
                cached_entry.input_tokens, cached_entry.output_tokens);
            cached_entry.response
        } else {
            // 缓存未命中，生成新摘要
            log::info!("[压缩] 缓存未命中，调用 LLM 生成摘要...");
            let (summary, input_tokens, output_tokens) =
                generate_summary_with_tokens(http, access_token, region, &to_compress, model_id).await?;

            // 保存到缓存
            if let Some(cache_ref) = cache.as_mut() {
                cache_ref.put(
                    sid,
                    &messages_hash,
                    summary.clone(),
                    input_tokens,
                    output_tokens,
                    message_count,
                    total_chars,
                );
                log::info!("[压缩] 摘要已保存到缓存（输入 {} tokens，输出 {} tokens）",
                    input_tokens, output_tokens);
            }

            summary
        }
    } else {
        // 没有缓存，直接生成摘要
        log::info!("[压缩] 未启用缓存，调用 LLM 生成摘要...");
        let (summary, _, _) = generate_summary_with_tokens(http, access_token, region, &to_compress, model_id).await?;
        summary
    };

    log::info!("[压缩] 摘要生成成功，长度: {} 字符", summary.len());

    // 构建新的消息列表
    let mut new_messages = system_messages;

    // 添加摘要消息
    new_messages.push(NormalizedMessage {
        role: "assistant".to_string(),
        content: Some(Value::String(format!(
            "[对话历史摘要]\n\n{}",
            summary
        ))),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
    });

    // 添加最近的消息
    new_messages.extend(recent_messages);

    // ✅ 验证并修复消息格式（避免 400 错误）
    new_messages = ensure_valid_message_sequence(new_messages);

    *messages = new_messages;
    log::info!("[压缩] 压缩完成，新消息数: {}", messages.len());
    Ok(true)
}

/// 计算消息列表的哈希值
fn calculate_messages_hash(messages: &[NormalizedMessage]) -> String {
    let mut hasher = DefaultHasher::new();

    for msg in messages {
        msg.role.hash(&mut hasher);

        // 优化：避免重复的 to_string()
        if let Some(content) = &msg.content {
            match content {
                Value::String(s) => s.hash(&mut hasher),
                Value::Array(arr) => {
                    for item in arr {
                        item.to_string().hash(&mut hasher);
                    }
                }
                other => other.to_string().hash(&mut hasher),
            }
        }

        // 包含 tool_calls 到哈希中（避免遗漏）
        if let Some(tool_calls) = &msg.tool_calls {
            for tc in tool_calls {
                tc.function.name.hash(&mut hasher);
                tc.function.arguments.hash(&mut hasher);
            }
        }
    }

    format!("{:x}", hasher.finish())
}

/// 调用 LLM 生成对话摘要（返回摘要和 token 统计）
async fn generate_summary_with_tokens(
    http: &reqwest::Client,
    access_token: &str,
    region: &str,
    messages: &[NormalizedMessage],
    model_id: &str,
) -> Result<(String, i32, i32), String> {
    let (summary, api_input_tokens, api_output_tokens) =
        generate_summary(http, access_token, region, messages, model_id).await?;

    // 优先使用 API 返回的 token，fallback 到本地估算
    let input_tokens = if let Some(tokens) = api_input_tokens {
        log::info!("[压缩] 使用 API 返回的输入 token: {}", tokens);
        tokens
    } else {
        let estimated = estimate_tokens_for_messages(messages);
        log::info!("[压缩] API 未返回 token，使用本地估算输入 token: {}", estimated);
        estimated
    };

    let output_tokens = if let Some(tokens) = api_output_tokens {
        log::info!("[压缩] 使用 API 返回的输出 token: {}", tokens);
        tokens
    } else {
        let estimated = (summary.len() / 4) as i32; // 粗略估算：4 字符 ≈ 1 token
        log::info!("[压缩] API 未返回 token，使用本地估算输出 token: {}", estimated);
        estimated
    };

    Ok((summary, input_tokens, output_tokens))
}

/// 估算消息的 token 数量
fn estimate_tokens_for_messages(messages: &[NormalizedMessage]) -> i32 {
    let total_chars: usize = messages.iter()
        .filter_map(|m| m.content.as_ref())
        .map(|c| c.to_string().len())
        .sum();
    (total_chars / 4) as i32 // 粗略估算：4 字符 ≈ 1 token
}

/// 调用 LLM 生成对话摘要（返回摘要和可能的 token 信息）
async fn generate_summary(
    http: &reqwest::Client,
    access_token: &str,
    region: &str,
    messages: &[NormalizedMessage],
    model_id: &str,
) -> Result<(String, Option<i32>, Option<i32>), String> {
    // 构建摘要提示词
    let conversation_text = format_messages_for_summary(messages);

    let summary_prompt = format!(
        r#"[系统指令：这是一个自动摘要请求，不是用户消息]

请为以下对话历史生成结构化摘要。要求：

1. 使用第三人称，不要用对话口吻
2. 使用项目符号列表格式
3. 过滤掉寒暄、客套话等无关内容
4. 重点记录：
   - 讨论的主要话题和问题
   - 执行的工具调用及结果
   - 分享的代码或技术信息
   - 已解决的问题和方案

输出格式：

## 对话摘要
* 话题1：关键信息
* 话题2：关键信息

## 工具执行
* 工具X：结果Y

## 代码实现
* 实现1：说明

## 已解决问题
* 问题1：解决方案

---

待摘要的对话内容：

{}
"#,
        conversation_text
    );

    // 构建请求
    let summary_request = vec![NormalizedMessage {
        role: "user".to_string(),
        content: Some(Value::String(summary_prompt)),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
    }];

    // 调用 LLM
    log::info!("[压缩] 调用 LLM 生成摘要...");

    let payload = build_kiro_payload(
        http,
        &crate::gateway::models::NormalizedRequest {
            model: model_id.to_string(),
            messages: summary_request,
            stream: false,
            max_tokens: Some(2000),
            temperature: Some(0.3),
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
            previous_response_id: None,
            thinking: None,
            tool_name_map: std::collections::HashMap::new(),
        },
        None,
        None,
    )
    .await
    .map_err(|e| format!("构建摘要请求失败: {}", e))?;

    // 发送请求
    let upstream_url = format!(
        "{}/generateAssistantResponse",
        build_q_service_url(region)
    );

    let response = http
        .post(&upstream_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/vnd.amazon.eventstream")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("发送摘要请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("摘要请求失败 ({}): {}", status, body));
    }

    // 解析响应
    let body = response
        .bytes()
        .await
        .map_err(|e| format!("读取摘要响应失败: {}", e))?;

    // 解析 EventStream 响应（现在返回 token 信息）
    let (summary, input_tokens, output_tokens) = parse_summary_from_eventstream(&body)?;

    Ok((summary, input_tokens, output_tokens))
}

/// 格式化消息用于摘要
fn format_messages_for_summary(messages: &[NormalizedMessage]) -> String {
    let mut result = String::new();

    for (idx, msg) in messages.iter().enumerate() {
        let role = match msg.role.as_str() {
            "user" => "用户",
            "assistant" => "助手",
            "tool" => "工具",
            _ => "系统",
        };

        result.push_str(&format!("\n[消息 {}] {}:\n", idx + 1, role));

        if let Some(content) = &msg.content {
            let text = match content {
                Value::String(s) => s.clone(),
                Value::Array(arr) => {
                    arr.iter()
                        .filter_map(|v| {
                            v.get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                _ => content.to_string(),
            };

            // 限制每条消息的长度
            let truncated = if text.len() > 1000 {
                format!("{}...[已截断]", &text[..crate::gateway::proxy::safe_truncate(&text, 1000)])
            } else {
                text
            };

            result.push_str(&truncated);
            result.push('\n');
        }

        if let Some(tool_calls) = &msg.tool_calls {
            for tc in tool_calls {
                result.push_str(&format!(
                    "  [工具调用] {}: {}\n",
                    tc.function.name,
                    tc.function.arguments
                ));
            }
        }
    }

    result
}

/// 确保消息序列符合 Kiro API 要求（避免 400 错误）
///
/// 规则：
/// 1. 必须以 user 消息开始
/// 2. 必须以 user 消息结束
/// 3. user 和 assistant 消息必须交替出现
/// 4. 不能有空的 user 消息
fn ensure_valid_message_sequence(messages: Vec<NormalizedMessage>) -> Vec<NormalizedMessage> {
    let mut result = Vec::new();
    let mut last_role: Option<String> = None;

    log::info!("[压缩] 开始验证消息格式，原始消息数: {}", messages.len());

    for msg in messages {
        // 跳过系统消息（系统消息不参与交替规则）
        if msg.role == "system" {
            result.push(msg);
            continue;
        }

        // 检查是否与上一条消息角色相同
        if let Some(ref last) = last_role {
            if last == &msg.role {
                // 连续相同角色，插入占位符消息
                let placeholder = if msg.role == "user" {
                    // 连续 user，插入 assistant 占位符
                    log::warn!("[压缩] 检测到连续 user 消息，插入 assistant 占位符");
                    NormalizedMessage {
                        role: "assistant".to_string(),
                        content: Some(Value::String("收到。".to_string())),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    }
                } else {
                    // 连续 assistant，插入 user 占位符
                    log::warn!("[压缩] 检测到连续 assistant 消息，插入 user 占位符");
                    NormalizedMessage {
                        role: "user".to_string(),
                        content: Some(Value::String("继续。".to_string())),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    }
                };
                result.push(placeholder);
            }
        }

        // 检查 user 消息是否为空
        if msg.role == "user" {
            let is_empty = match &msg.content {
                Some(Value::String(s)) => s.trim().is_empty(),
                Some(Value::Array(arr)) => arr.is_empty(),
                None => true,
                _ => false,
            };

            if is_empty && msg.tool_call_id.is_none() {
                log::warn!("[压缩] 跳过空的 user 消息");
                continue;
            }
        }

        result.push(msg.clone());
        last_role = Some(msg.role);
    }

    // 确保以 user 消息开始
    if let Some(first) = result.first() {
        if first.role != "system" && first.role != "user" {
            log::warn!("[压缩] 消息不以 user 开始，插入占位符");
            result.insert(0, NormalizedMessage {
                role: "user".to_string(),
                content: Some(Value::String("你好。".to_string())),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            });
        }
    }

    // 确保以 user 消息结束
    if let Some(last) = result.last() {
        if last.role != "user" {
            log::warn!("[压缩] 消息不以 user 结束，追加占位符");
            result.push(NormalizedMessage {
                role: "user".to_string(),
                content: Some(Value::String("继续。".to_string())),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            });
        }
    }

    log::info!("[压缩] 消息格式验证完成，最终消息数: {}", result.len());
    result
}

/// 从 EventStream 响应中解析摘要（尝试提取 token 信息）
fn parse_summary_from_eventstream(body: &[u8]) -> Result<(String, Option<i32>, Option<i32>), String> {
    let mut summary = String::new();
    let mut input_tokens: Option<i32> = None;
    let mut output_tokens: Option<i32> = None;
    let mut offset = 0;

    while offset < body.len() {
        match decode_message(&body[offset..]) {
            Ok(Some((event, consumed))) => {
                offset += consumed;

                // 解析 payload 为 JSON
                if let Ok(value) = serde_json::from_slice::<Value>(&event.payload) {
                    // 提取文本内容
                    if let Some(content) = value
                        .get("assistantResponseEvent")
                        .and_then(|e| e.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        summary.push_str(content);
                    }

                    // 尝试提取 token 信息（即使可能不存在）
                    // 方式1：metadataEvent.tokenUsage（新格式）
                    if let Some(metadata_event) = value.get("metadataEvent") {
                        if let Some(token_usage) = metadata_event.get("tokenUsage") {
                            let uncached = token_usage
                                .get("uncachedInputTokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;

                            // input_tokens 仅为未缓存的输入（Anthropic 规范，避免双重计费）
                            input_tokens = Some(uncached);
                            output_tokens = token_usage
                                .get("outputTokens")
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);

                            log::info!("[压缩] ✅ 从 metadataEvent 提取到 token: 输入={:?}, 输出={:?}",
                                input_tokens, output_tokens);
                        }
                    }

                    // 方式2：顶层 usage 字段（旧格式）
                    if input_tokens.is_none() {
                        if let Some(usage) = value.get("usage") {
                            input_tokens = usage
                                .get("inputTokens")
                                .or_else(|| usage.get("input_tokens"))
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);
                            output_tokens = usage
                                .get("outputTokens")
                                .or_else(|| usage.get("output_tokens"))
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);

                            if input_tokens.is_some() || output_tokens.is_some() {
                                log::info!("[压缩] ✅ 从 usage 提取到 token: 输入={:?}, 输出={:?}",
                                    input_tokens, output_tokens);
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                break;
            }
            Err(e) => {
                log::warn!("[压缩] 解析 EventStream 失败: {}", e);
                break;
            }
        }
    }

    if summary.is_empty() {
        Err("未能从响应中提取摘要".to_string())
    } else {
        Ok((summary, input_tokens, output_tokens))
    }
}
