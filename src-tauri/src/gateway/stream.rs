
#[derive(Debug, Clone, PartialEq)]
pub enum KiroEvent {
    Text(String),
    Thinking(String),
    ThinkingSignature(String),
    ToolUseStart {
        id: String,
        name: String,
    },
    ToolUseInputDelta {
        id: String,
        name: Option<String>,
        input_delta: String,
    },
    ToolUseStop {
        id: String,
    },
    Usage {
        input_tokens: i32,
        output_tokens: i32,
        cache_read_input_tokens: Option<i32>,
        cache_creation_input_tokens: Option<i32>,
    },
    ContextUsage {
        percentage: f32,
    },
    Metering {
        unit: String,
        unit_plural: String,
        usage: f64,
    },
    Citation {
        text: Option<String>,
        link: String,
        target: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregatedCitation {
    pub text: Option<String>,
    pub link: String,
    pub target: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AggregatedKiroResponse {
    pub text: String,
    pub thinking: String,
    pub thinking_signature: Option<String>,
    pub tool_calls: Vec<(String, String, String)>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_input_tokens: Option<i32>,
    pub cache_creation_input_tokens: Option<i32>,
    pub context_usage_percentage: Option<f32>,
    pub metering_usage: Option<f64>,
    pub citations: Vec<AggregatedCitation>,
}


pub fn parse_kiro_event_full(json_str: &str) -> Option<KiroEvent> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;

    // 添加调试：只记录真正的 token/usage 事件（meteringEvent、contextUsageEvent、metadataEvent）
    let json_lower = json_str.to_lowercase();
    let is_metering_or_usage_event = json_lower.contains("\"meteringevent\"")
        || json_lower.contains("\"contextusageevent\"")
        || json_lower.contains("\"metadataevent\"")
        || (json_lower.contains("\"usage\"") && json_lower.contains("\"inputtokens\""));

    if is_metering_or_usage_event {
        log::debug!("[Token 解析] 发现 token/usage 事件: {}",
            if json_str.len() > 500 {
                format!("{}...", &json_str[..crate::gateway::proxy::safe_truncate(json_str, 500)])
            } else {
                json_str.to_string()
            }
        );
    }

    // 解析 metadataEvent 中的 tokenUsage
    // Kiro IDE 的 token 信息在 metadataEvent.tokenUsage 中
    if let Some(metadata_event) = value.get("metadataEvent").and_then(|item| item.as_object()) {
        log::debug!("[Token 解析] 发现 metadataEvent: {:?}", metadata_event.keys().collect::<Vec<_>>());

        if let Some(token_usage) = metadata_event.get("tokenUsage").and_then(|item| item.as_object()) {
            // Kiro IDE 的字段名：
            // - uncachedInputTokens (未缓存的输入 tokens)
            // - cacheReadInputTokens (缓存读取 tokens)
            // - cacheWriteInputTokens (缓存写入 tokens)
            // - outputTokens (输出 tokens)
            // - totalTokens (总 tokens)
            let uncached_input_tokens = token_usage
                .get("uncachedInputTokens")
                .and_then(|item| item.as_i64())
                .unwrap_or(0) as i32;
            let cache_read_input_tokens = token_usage
                .get("cacheReadInputTokens")
                .and_then(|item| item.as_i64())
                .map(|v| v as i32);
            let cache_creation_input_tokens = token_usage
                .get("cacheWriteInputTokens")
                .and_then(|item| item.as_i64())
                .map(|v| v as i32);
            let output_tokens = token_usage
                .get("outputTokens")
                .and_then(|item| item.as_i64())
                .unwrap_or(0) as i32;

            // input_tokens 仅为未缓存的输入（符合 Anthropic 官方规范）
            // 客户端会单独看到 cache_read_input_tokens 和 cache_creation_input_tokens 字段
            // 三者相加才是真实总输入量。如果这里包含 cache_*，客户端账单会双重计费虚高
            // 参考：chaogei/Kiro-account-manager v1.6.6 修复
            let input_tokens = uncached_input_tokens;

            // 添加调试日志：记录原始 tokenUsage JSON
            log::info!(
                "[Token 解析] ✅ 发现 metadataEvent.tokenUsage: {}",
                serde_json::to_string(token_usage).unwrap_or_else(|_| "invalid".to_string())
            );
            log::info!(
                "[Token 解析] 已解析: 未缓存输入={}, 缓存读取={:?}, 缓存写入={:?}, 输出={}, 总输入={}",
                uncached_input_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
                output_tokens,
                input_tokens
            );

            if input_tokens > 0 || output_tokens > 0 {
                return Some(KiroEvent::Usage {
                    input_tokens,
                    output_tokens,
                    cache_read_input_tokens,
                    cache_creation_input_tokens,
                });
            } else {
                log::warn!("[Token 解析] ⚠️ 发现 tokenUsage 但所有 token 都为 0");
            }
        } else {
            log::warn!("[Token 解析] ⚠️ 发现 metadataEvent 但没有 tokenUsage 字段");
        }
    }

    // 兼容旧格式：解析顶层的 usage 字段（用于其他格式的响应）
    if let Some(usage) = value.get("usage").and_then(|item| item.as_object()) {
        let input_tokens = usage
            .get("inputTokens")
            .or_else(|| usage.get("input_tokens"))
            .and_then(|item| item.as_i64())
            .unwrap_or(0) as i32;
        let output_tokens = usage
            .get("outputTokens")
            .or_else(|| usage.get("output_tokens"))
            .and_then(|item| item.as_i64())
            .unwrap_or(0) as i32;

        let cache_read_input_tokens = usage
            .get("cachedReadTokens")
            .or_else(|| usage.get("cacheReadInputTokens"))
            .or_else(|| usage.get("cache_read_input_tokens"))
            .and_then(|item| item.as_i64())
            .map(|v| v as i32);
        let cache_creation_input_tokens = usage
            .get("cachedWriteTokens")
            .or_else(|| usage.get("cacheCreationInputTokens"))
            .or_else(|| usage.get("cache_creation_input_tokens"))
            .and_then(|item| item.as_i64())
            .map(|v| v as i32);

        log::info!(
            "[Token 解析] 原始 usage JSON (旧格式): {}",
            serde_json::to_string(usage).unwrap_or_else(|_| "invalid".to_string())
        );

        if input_tokens > 0 || output_tokens > 0 || cache_read_input_tokens.is_some() || cache_creation_input_tokens.is_some() {
            return Some(KiroEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
            });
        }
    }

    // 解析 contextUsageEvent
    if let Some(context_event) = value.get("contextUsageEvent").and_then(|item| item.as_object()) {
        if let Some(percentage) = context_event.get("contextUsagePercentage").and_then(|item| item.as_f64()) {
            return Some(KiroEvent::ContextUsage {
                percentage: percentage as f32,
            });
        }
    }

    if let Some(metering) = value.get("meteringEvent").and_then(|item| item.as_object()) {
        let unit = metering
            .get("unit")
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string();
        let unit_plural = metering
            .get("unitPlural")
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string();
        let usage = metering
            .get("usage")
            .and_then(|item| item.as_f64())
            .unwrap_or(0.0);

        if !unit.is_empty() {
            return Some(KiroEvent::Metering {
                unit,
                unit_plural,
                usage,
            });
        }
    }
    if let Some(text) = parse_reasoning_text(&value) {
        if !text.is_empty() {
            return Some(KiroEvent::Thinking(text));
        }
    }
    // 提取 reasoning signature（可能在单独的事件中，也可能和 text 一起）
    if let Some(sig) = parse_reasoning_signature(&value) {
        if !sig.is_empty() {
            return Some(KiroEvent::ThinkingSignature(sig));
        }
    }

    // 解析 codeReferenceEvent
    if let Some(code_ref) = value.get("codeReferenceEvent") {
        if let Some(citation) = parse_code_reference_event(code_ref) {
            return Some(KiroEvent::Citation {
                text: citation.text,
                link: citation.link,
                target: citation.target,
            });
        }
    }

    // 解析 supplementaryWebLinksEvent
    if let Some(web_links) = value.get("supplementaryWebLinksEvent") {
        if let Some(citation) = parse_supplementary_web_links_event(web_links) {
            return Some(KiroEvent::Citation {
                text: citation.text,
                link: citation.link,
                target: citation.target,
            });
        }
    }

    if let Some(citation) = parse_citation_event(&value) {
        return Some(KiroEvent::Citation {
            text: citation.text,
            link: citation.link,
            target: citation.target,
        });
    }

    if let Some(tool_use_id) = value.get("toolUseId").and_then(|item| item.as_str())
        .or_else(|| value.get("toolUseEvent").and_then(|e| e.get("toolUseId")).and_then(|item| item.as_str()))
    {
        // 支持 toolUseEvent 包装格式
        let tool_data = value.get("toolUseEvent").unwrap_or(&value);

        let name = tool_data
            .get("name")
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string();

        if tool_data.get("stop").and_then(|item| item.as_bool()) == Some(true) {
            return Some(KiroEvent::ToolUseStop {
                id: tool_use_id.to_string(),
            });
        }

        if let Some(input) = tool_data.get("input") {
            let input_delta = if let Some(text) = input.as_str() {
                log::debug!("[Tool Use] input 是字符串: {}", text.chars().take(100).collect::<String>());
                text.to_string()
            } else if input.is_object() || input.is_array() {
                let serialized = serde_json::to_string(input).unwrap_or_default();
                log::debug!("[Tool Use] input 是对象/数组，序列化后: {}", serialized.chars().take(100).collect::<String>());
                serialized
            } else {
                log::warn!("[Tool Use] input 类型未知: {:?}", input);
                String::new()
            };
            if !input_delta.is_empty() {
                log::debug!("[Tool Use] 发送 ToolUseInputDelta: id={}, delta_len={}", tool_use_id, input_delta.len());
                return Some(KiroEvent::ToolUseInputDelta {
                    id: tool_use_id.to_string(),
                    name: if name.is_empty() { None } else { Some(name) },
                    input_delta,
                });
            }
        }

        if !name.is_empty() {
            log::debug!("[Tool Use] 发送 ToolUseStart: id={}, name={}", tool_use_id, name);
            return Some(KiroEvent::ToolUseStart {
                id: tool_use_id.to_string(),
                name,
            });
        }
    }

    if let Some(tool) = value
        .get("assistantResponseEvent")
        .and_then(|item| item.get("toolUses"))
        .and_then(|item| item.as_array())
        .and_then(|items| items.first())
    {
        let id = tool
            .get("toolUseId")
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string();
        let name = tool
            .get("name")
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string();

        if !name.is_empty() {
            return Some(KiroEvent::ToolUseStart { id, name });
        }
    }

    parse_text_content(&value).map(KiroEvent::Text)
}

pub fn deduplicate_tool_calls(
    tool_calls: Vec<(String, String, String)>,
) -> Vec<(String, String, String)> {
    use std::collections::HashMap;

    if tool_calls.is_empty() {
        return tool_calls;
    }

    let mut by_id: HashMap<String, (String, String, String)> = HashMap::new();
    let mut order = Vec::new();

    for (id, name, args) in tool_calls {
        if !by_id.contains_key(&id) {
            order.push(id.clone());
        }
        by_id.insert(id.clone(), (id, name, args));
    }

    order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect()
}

/// 从已切分好的 JSON payload 数组直接聚合（每帧直接解析，无需 extract_json 重新切分）
pub fn aggregate_kiro_response_from_payloads(payloads: &[String]) -> AggregatedKiroResponse {
    let mut aggregated = AggregatedKiroResponse::default();
    let mut tool_accumulators: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    let mut found_usage = false;
    let mut json_count = 0;
    let mut event_counts = std::collections::HashMap::new();

    log::info!("[聚合] 开始聚合，共 {} 个 payload", payloads.len());

    for json_str in payloads {
        let json_str = json_str.trim();
        if json_str.is_empty() {
            continue;
        }
        json_count += 1;

        log::debug!("[聚合] JSON #{}: {}", json_count, json_str.chars().take(200).collect::<String>());

        if let Some(event) = parse_kiro_event_full(json_str) {
            let event_type = match &event {
                KiroEvent::Text(_) => "Text",
                KiroEvent::Thinking(_) => "Thinking",
                KiroEvent::ThinkingSignature(_) => "ThinkingSignature",
                KiroEvent::ToolUseStart { .. } => "ToolUseStart",
                KiroEvent::ToolUseInputDelta { .. } => "ToolUseInputDelta",
                KiroEvent::ToolUseStop { .. } => "ToolUseStop",
                KiroEvent::Usage { .. } => "Usage",
                KiroEvent::ContextUsage { .. } => "ContextUsage",
                KiroEvent::Metering { .. } => "Metering",
                KiroEvent::Citation { .. } => "Citation",
            };
            *event_counts.entry(event_type).or_insert(0) += 1;

            match event {
                KiroEvent::Text(text) => {
                    log::debug!("[聚合] 文本事件: {} 字符", text.len());
                    aggregated.text.push_str(&text);
                }
                KiroEvent::Thinking(text) => {
                    log::debug!("[聚合] 思考事件: {} 字符", text.len());
                    aggregated.thinking.push_str(&text);
                }
                KiroEvent::ThinkingSignature(sig) => {
                    log::debug!("[聚合] 思考签名: {} 字符", sig.len());
                    aggregated.thinking_signature = Some(sig);
                }
                KiroEvent::ToolUseStart { id, name } => {
                    log::debug!("[聚合] 工具使用开始: id={}, name={}", id, name);
                    let entry = tool_accumulators
                        .entry(id)
                        .or_insert((String::new(), String::new()));
                    if entry.0.is_empty() {
                        entry.0 = name;
                    }
                }
                KiroEvent::ToolUseInputDelta {
                    id,
                    name,
                    input_delta,
                } => {
                    log::debug!("[聚合] 工具输入增量: id={}, delta_len={}", id, input_delta.len());
                    if let Some((existing_name, current_input)) = tool_accumulators.get_mut(&id) {
                        if existing_name.is_empty() {
                            if let Some(name) = name {
                                *existing_name = name;
                            }
                        }
                        if input_delta.trim_start().starts_with('{') && current_input.trim_start().starts_with('{') {
                            log::debug!("[聚合] 检测到完整 JSON input，替换而不是追加");
                            *current_input = input_delta;
                        } else {
                            current_input.push_str(&input_delta);
                        }
                    } else {
                        tool_accumulators.insert(id, (name.unwrap_or_default(), input_delta));
                    }
                }
                KiroEvent::ToolUseStop { id } => {
                    log::debug!("[聚合] 工具使用结束: id={}", id);
                    if let Some((name, input)) = tool_accumulators.remove(&id) {
                        log::debug!("[聚合] 完整的工具调用: id={}, name={}, input_len={}", id, name, input.len());
                        aggregated.tool_calls.push((id, name, input));
                    } else {
                        log::warn!("[聚合] ⚠️  工具使用结束但未找到对应的累加器: id={}", id);
                    }
                }
                KiroEvent::Usage {
                    input_tokens,
                    output_tokens,
                    cache_read_input_tokens,
                    cache_creation_input_tokens,
                } => {
                    found_usage = true;
                    aggregated.input_tokens = input_tokens;
                    aggregated.output_tokens = output_tokens;
                    aggregated.cache_read_input_tokens = cache_read_input_tokens;
                    aggregated.cache_creation_input_tokens = cache_creation_input_tokens;
                    log::info!(
                        "[聚合] ✅ 发现 usage 信息: input={}, output={}, cache_read={:?}, cache_creation={:?}",
                        input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens
                    );
                }
                KiroEvent::ContextUsage { percentage } => {
                    log::debug!("[聚合] 上下文使用: {}%", percentage);
                    aggregated.context_usage_percentage = Some(percentage);
                }
                KiroEvent::Metering { usage, .. } => {
                    log::debug!("[聚合] 计量: {}", usage);
                    aggregated.metering_usage = Some(usage);
                }
                KiroEvent::Citation { text, link, target } => {
                    log::debug!("[聚合] 引用: link={}", link);
                    aggregated.citations.push(AggregatedCitation { text, link, target });
                }
            }
        } else {
            log::warn!("[聚合] 解析事件失败: {}", json_str.chars().take(200).collect::<String>());
        }
    }

    // 收集未关闭的 tool_accumulators
    for (id, (name, input)) in tool_accumulators {
        if !name.is_empty() || !input.is_empty() {
            log::warn!("[聚合] ⚠️ 未关闭的工具调用: id={}, name={}", id, name);
            aggregated.tool_calls.push((id, name, input));
        }
    }

    aggregated.tool_calls = deduplicate_tool_calls(aggregated.tool_calls);

    log::info!("[聚合] 完成: 处理了 {} 个 JSON 对象", json_count);
    log::info!("[聚合] 事件计数: {:?}", event_counts);
    log::info!(
        "[聚合] 结果: 文本={} 字符, 思考={} 字符, 工具调用={}, 引用={}",
        aggregated.text.len(), aggregated.thinking.len(),
        aggregated.tool_calls.len(), aggregated.citations.len()
    );

    if !found_usage {
        log::warn!("[聚合] ⚠️ Kiro 响应中未找到 usage 信息!");
    }

    aggregated
}



fn parse_text_content(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.get("content").and_then(|item| item.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    if let Some(text) = value
        .get("delta")
        .and_then(|item| item.get("text"))
        .and_then(|item| item.as_str())
    {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    if let Some(text) = value
        .get("contentBlockDelta")
        .and_then(|item| item.get("delta"))
        .and_then(|item| item.get("text"))
        .and_then(|item| item.as_str())
    {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    if let Some(text) = value
        .get("assistantResponseEvent")
        .and_then(|item| item.get("content"))
        .and_then(|item| item.as_str())
    {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    None
}

fn parse_reasoning_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value
        .get("reasoningContentEvent")
        .and_then(|item| item.get("text"))
        .and_then(|item| item.as_str())
    {
        return Some(text.to_string());
    }

    if let Some(text) = value
        .get("delta")
        .and_then(|item| item.get("thinking"))
        .and_then(|item| item.as_str())
    {
        return Some(text.to_string());
    }

    if let Some(text) = value
        .get("contentBlockDelta")
        .and_then(|item| item.get("delta"))
        .and_then(|item| item.get("thinking"))
        .and_then(|item| item.as_str())
    {
        return Some(text.to_string());
    }

    None
}

/// 从 reasoningContentEvent 中提取 signature
fn parse_reasoning_signature(value: &serde_json::Value) -> Option<String> {
    value
        .get("reasoningContentEvent")
        .and_then(|item| item.get("signature"))
        .and_then(|item| item.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// 解析 codeReferenceEvent
fn parse_code_reference_event(value: &serde_json::Value) -> Option<AggregatedCitation> {
    let references = value.get("references")?.as_array()?;
    let first_ref = references.first()?;

    let repository = first_ref
        .get("repository")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let license_name = first_ref
        .get("licenseName")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 构造 link（使用 repository 作为链接）
    let link = if !repository.is_empty() {
        repository.to_string()
    } else {
        return None;
    };

    // 构造 text（显示许可证信息）
    let text = if !license_name.is_empty() {
        Some(format!("Code reference ({})", license_name))
    } else {
        Some("Code reference".to_string())
    };

    // 构造 target（保留原始的 recommendationContentSpan）
    let target = first_ref
        .get("recommendationContentSpan")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    Some(AggregatedCitation { text, link, target })
}

/// 解析 supplementaryWebLinksEvent
fn parse_supplementary_web_links_event(value: &serde_json::Value) -> Option<AggregatedCitation> {
    let links = value.get("supplementaryWebLinks")?.as_array()?;
    let first_link = links.first()?;

    let url = first_link
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?
        .to_string();

    let title = first_link
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let snippet = first_link
        .get("snippet")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // 使用 title 或 snippet 作为显示文本
    let text = title.or(snippet);

    // 构造 target（保留原始的 url 和 snippet）
    let target = serde_json::json!({
        "url": url,
        "snippet": first_link.get("snippet").cloned().unwrap_or(serde_json::Value::Null)
    });

    Some(AggregatedCitation {
        text,
        link: url,
        target,
    })
}

fn parse_citation_event(value: &serde_json::Value) -> Option<AggregatedCitation> {
    let target = value.get("target")?.clone();
    let link = value
        .get("citationLink")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())?
        .to_string();
    let text = value
        .get("citationText")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    ensure_citation_target_supported(&target)?;

    Some(AggregatedCitation { text, link, target })
}

fn ensure_citation_target_supported(target: &serde_json::Value) -> Option<()> {
    if let Some(range) = target.get("range") {
        let start_index = range.get("start").and_then(|item| item.as_u64())? as usize;
        let end_index = range.get("end").and_then(|item| item.as_u64())? as usize;
        if end_index < start_index {
            return None;
        }
        return Some(());
    }

    if target
        .get("location")
        .and_then(|item| item.as_u64())
        .is_some()
    {
        return Some(());
    }

    None
}

use crate::gateway::models::{
    OpenAIChatChunk, OpenAIChatChunkChoice, OpenAIChatDelta, OpenAIChatResponse,
    OpenAIChatChoice, OpenAIChatResponseMessage, OpenAIResponseToolCall, OpenAIChatUsage,
    OpenAIToolCallFunction, OpenAIUsage,
};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub fn build_openai_chunk(
    completion_id: &str,
    created: i64,
    model: &str,
    delta: OpenAIChatDelta,
    finish_reason: Option<String>,
    usage: Option<OpenAIChatUsage>,
) -> OpenAIChatChunk {
    OpenAIChatChunk {
        id: completion_id.to_string(),
        object: "chat.completion.chunk".to_string(),
        created,
        model: model.to_string(),
        choices: vec![OpenAIChatChunkChoice {
            index: 0,
            delta,
            finish_reason,
        }],
        usage,
    }
}

pub fn build_openai_response(
    model: &str,
    aggregated: &AggregatedKiroResponse,
) -> OpenAIChatResponse {
    let completion_id = format!("chatcmpl-{}", Uuid::new_v4().simple());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut message = OpenAIChatResponseMessage {
        role: "assistant".to_string(),
        content: if aggregated.text.is_empty() {
            None
        } else {
            Some(aggregated.text.clone())
        },
        tool_calls: None,
    };

    let finish_reason = if !aggregated.tool_calls.is_empty() {
        let tool_calls: Vec<OpenAIResponseToolCall> = aggregated
            .tool_calls
            .iter()
            .map(|(id, name, arguments)| OpenAIResponseToolCall {
                id: id.clone(),
                call_type: "function".to_string(),
                function: OpenAIToolCallFunction {
                    name: name.clone(),
                    arguments: arguments.clone(),
                },
            })
            .collect();
        message.tool_calls = Some(tool_calls);
        "tool_calls".to_string()
    } else {
        "stop".to_string()
    };

    OpenAIChatResponse {
        id: completion_id,
        object: "chat.completion".to_string(),
        created,
        model: model.to_string(),
        choices: vec![OpenAIChatChoice {
            index: 0,
            message,
            finish_reason: Some(finish_reason),
        }],
        usage: OpenAIUsage {
            input_tokens: aggregated.input_tokens,
            output_tokens: aggregated.output_tokens,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kiro_event_full_reads_text_tool_and_usage_events() {
        assert_eq!(
            parse_kiro_event_full(r#"{"assistantResponseEvent":{"content":"hello"}}"#),
            Some(KiroEvent::Text("hello".to_string()))
        );
        assert_eq!(
            parse_kiro_event_full(r#"{"toolUseId":"tool_1","name":"search_docs"}"#),
            Some(KiroEvent::ToolUseStart {
                id: "tool_1".to_string(),
                name: "search_docs".to_string(),
            })
        );
        assert_eq!(
            parse_kiro_event_full(r#"{"toolUseId":"tool_1","input":{"q":"gateway"}}"#),
            Some(KiroEvent::ToolUseInputDelta {
                id: "tool_1".to_string(),
                name: None,
                input_delta: "{\"q\":\"gateway\"}".to_string(),
            })
        );
        assert_eq!(
            parse_kiro_event_full(r#"{"toolUseId":"tool_1","stop":true}"#),
            Some(KiroEvent::ToolUseStop {
                id: "tool_1".to_string(),
            })
        );
        assert_eq!(
            parse_kiro_event_full(r#"{"usage":{"inputTokens":12,"outputTokens":34}}"#),
            Some(KiroEvent::Usage {
                input_tokens: 12,
                output_tokens: 34,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            })
        );
    }

    #[test]
    fn parse_kiro_event_full_reads_reasoning_content() {
        assert_eq!(
            parse_kiro_event_full(r#"{"reasoningContentEvent":{"text":"分析中"}}"#),
            Some(KiroEvent::Thinking("分析中".to_string()))
        );
    }

    #[test]
    fn parse_kiro_event_full_reads_metering_event() {
        assert_eq!(
            parse_kiro_event_full(r#"{"meteringEvent":{"unit":"credit","unitPlural":"credits","usage":0.3876425741791045}}"#),
            Some(KiroEvent::Metering {
                unit: "credit".to_string(),
                unit_plural: "credits".to_string(),
                usage: 0.3876425741791045,
            })
        );
    }

    #[test]
    fn parse_kiro_event_full_keeps_tool_name_with_wrapped_input() {
        assert_eq!(
            parse_kiro_event_full(
                r#"{"toolUseEvent":{"toolUseId":"tool_1","name":"server_health","input":"{}"}}"#
            ),
            Some(KiroEvent::ToolUseInputDelta {
                id: "tool_1".to_string(),
                name: Some("server_health".to_string()),
                input_delta: "{}".to_string(),
            })
        );
    }

    #[test]
    fn aggregate_kiro_response_from_payloads_keeps_name_when_input_arrives_first() {
        let payloads = vec![
            r#"{"toolUseEvent":{"toolUseId":"tool_1","name":"server_health","input":"{}"}}"#
                .to_string(),
            r#"{"toolUseEvent":{"toolUseId":"tool_1","stop":true}}"#.to_string(),
        ];

        let aggregated = aggregate_kiro_response_from_payloads(&payloads);

        assert_eq!(
            aggregated.tool_calls,
            vec![(
                "tool_1".to_string(),
                "server_health".to_string(),
                "{}".to_string()
            )]
        );
    }

    #[test]
    fn parse_kiro_event_full_reads_citation_events() {
        assert_eq!(
            parse_kiro_event_full(
                r#"{"target":{"range":{"start":2,"end":5}},"citationText":"Rust","citationLink":"https://example.com/rust"}"#
            ),
            Some(KiroEvent::Citation {
                text: Some("Rust".to_string()),
                link: "https://example.com/rust".to_string(),
                target: serde_json::json!({ "range": { "start": 2, "end": 5 } }),
            })
        );
        assert_eq!(
            parse_kiro_event_full(
                r#"{"target":{"location":6},"citationText":"Rust","citationLink":"https://example.com/location"}"#
            ),
            Some(KiroEvent::Citation {
                text: Some("Rust".to_string()),
                link: "https://example.com/location".to_string(),
                target: serde_json::json!({ "location": 6 }),
            })
        );
    }

    #[test]
    fn ensure_citation_target_supported_accepts_location_without_guessing_range() {
        assert_eq!(
            ensure_citation_target_supported(&serde_json::json!({ "location": 6 })),
            Some(())
        );
    }

    #[test]
    fn aggregate_kiro_response_from_payloads_collects_citations() {
        let payloads = vec![
            r#"{"assistantResponseEvent":{"content":"Hello Rust"}}"#.to_string(),
            r#"{"target":{"range":{"start":6,"end":10}},"citationText":"Rust","citationLink":"https://example.com/rust"}"#.to_string(),
        ];

        let aggregated = aggregate_kiro_response_from_payloads(&payloads);

        assert_eq!(aggregated.text, "Hello Rust");
        assert_eq!(
            aggregated.citations,
            vec![AggregatedCitation {
                text: Some("Rust".to_string()),
                link: "https://example.com/rust".to_string(),
                target: serde_json::json!({ "range": { "start": 6, "end": 10 } }),
            }]
        );
    }

    #[test]
    fn deduplicate_tool_calls_keeps_latest_args_per_id() {
        let input = vec![
            ("tool_1".to_string(), "search".to_string(), "{}".to_string()),
            (
                "tool_1".to_string(),
                "search".to_string(),
                "{\"q\":\"gateway\"}".to_string(),
            ),
            (
                "tool_2".to_string(),
                "open".to_string(),
                "{\"path\":\"README.md\"}".to_string(),
            ),
        ];

        let deduped = deduplicate_tool_calls(input);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].0, "tool_1");
        assert_eq!(deduped[0].2, "{\"q\":\"gateway\"}");
    }
}
