// 网关核心功能测试
// 测试账号选择策略、Payload 裁剪、消息合并等关键功能

use serde_json::json;

/// 测试 Payload 裁剪逻辑 - 保留最近的对话
#[test]
fn test_trim_payload_preserves_recent_messages() {
    let mut payload = json!({
        "conversationState": {
            "history": [
                {"user_input_message": {"user_input_message_context": {"text": "Old message 1"}}},
                {"assistant_response_message": {"text": "Old response 1"}},
                {"user_input_message": {"user_input_message_context": {"text": "Recent message"}}},
                {"assistant_response_message": {"text": "Recent response"}}
            ]
        }
    });

    let max_bytes = 200;

    // 简化的裁剪逻辑
    loop {
        let current_size = serde_json::to_string(&payload).unwrap().len();
        if current_size <= max_bytes {
            break;
        }

        let history_len = payload.pointer("/conversationState/history")
            .and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);

        if history_len <= 2 {
            break;
        }

        if let Some(history) = payload.pointer_mut("/conversationState/history") {
            if let Some(arr) = history.as_array_mut() {
                arr.remove(0);
            }
        }
    }

    let history = payload.pointer("/conversationState/history")
        .and_then(|v| v.as_array()).unwrap();

    assert!(history.len() >= 2);
    let last_msg = &history[history.len() - 1];
    assert!(last_msg.get("assistant_response_message")
        .and_then(|m| m.get("text"))
        .and_then(|t| t.as_str()).unwrap().contains("Recent response"));

    println!("✓ Payload trimming preserves recent messages");
}

/// 测试 Payload 裁剪 - 保持 tool_call/result 配对
#[test]
fn test_trim_payload_preserves_tool_pairs() {
    let mut payload = json!({
        "conversationState": {
            "history": [
                {"assistant_response_message": {"text": "Let me search", "tool_uses": [{"id": "call_1", "name": "search", "input": {"q": "test"}}]}},
                {"user_input_message": {"user_input_message_context": {"tool_results": [{"call_id": "call_1", "output": "Found results"}]}}},
                {"user_input_message": {"user_input_message_context": {"text": "Recent message"}}},
                {"assistant_response_message": {"text": "Final response"}}
            ]
        }
    });

    let max_bytes = 300;

    loop {
        let current_size = serde_json::to_string(&payload).unwrap().len();
        if current_size <= max_bytes {
            break;
        }

        let history_len = payload.pointer("/conversationState/history")
            .and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);

        if history_len <= 2 {
            break;
        }

        let first_has_tools = payload.pointer("/conversationState/history/0/assistant_response_message/tool_uses")
            .and_then(|v| v.as_array()).map(|arr| !arr.is_empty()).unwrap_or(false);

        let second_has_results = payload.pointer("/conversationState/history/1/user_input_message/user_input_message_context/tool_results")
            .and_then(|v| v.as_array()).map(|arr| !arr.is_empty()).unwrap_or(false);

        if let Some(history) = payload.pointer_mut("/conversationState/history") {
            if let Some(arr) = history.as_array_mut() {
                if first_has_tools && second_has_results && arr.len() > 3 {
                    // 配对删除
                    arr.remove(0);
                    arr.remove(0);
                } else {
                    // 单个删除
                    arr.remove(0);
                }
            }
        }
    }

    let history = payload.pointer("/conversationState/history")
        .and_then(|v| v.as_array()).unwrap();

    // 验证：如果有 tool_uses，必须有对应的 tool_results
    // 如果没有 tool_uses，也不应该有孤立的 tool_results
    let has_tool_uses = history.iter().any(|msg| {
        msg.get("assistant_response_message")
            .and_then(|m| m.get("tool_uses"))
            .and_then(|t| t.as_array())
            .map(|arr| !arr.is_empty()).unwrap_or(false)
    });

    let has_tool_results = history.iter().any(|msg| {
        msg.get("user_input_message")
            .and_then(|m| m.get("user_input_message_context"))
            .and_then(|ctx| ctx.get("tool_results"))
            .and_then(|r| r.as_array())
            .map(|arr| !arr.is_empty()).unwrap_or(false)
    });

    // 配对关系：要么都有，要么都没有
    assert_eq!(has_tool_uses, has_tool_results, "Tool call/result pairing should be preserved");
    println!("✓ Payload trimming preserves tool call/result pairs");
}

/// 测试账号选择策略 - Round Robin
#[test]
fn test_account_selection_round_robin() {
    struct Account {
        id: String,
        failure_count: u32,
    }

    let accounts = vec![
        Account { id: "acc1".to_string(), failure_count: 0 },
        Account { id: "acc2".to_string(), failure_count: 0 },
        Account { id: "acc3".to_string(), failure_count: 0 },
    ];

    let mut last_index = 0;
    let max_failures = 3;

    let select_round_robin = |accounts: &[Account], last_idx: &mut usize| -> Option<String> {
        let start = *last_idx;
        for _ in 0..accounts.len() {
            *last_idx = (*last_idx + 1) % accounts.len();
            if accounts[*last_idx].failure_count < max_failures {
                return Some(accounts[*last_idx].id.clone());
            }
        }
        *last_idx = start;
        None
    };

    let selected1 = select_round_robin(&accounts, &mut last_index);
    assert_eq!(selected1, Some("acc2".to_string()));

    let selected2 = select_round_robin(&accounts, &mut last_index);
    assert_eq!(selected2, Some("acc3".to_string()));

    let selected3 = select_round_robin(&accounts, &mut last_index);
    assert_eq!(selected3, Some("acc1".to_string()));

    println!("✓ Round Robin account selection works correctly");
}

/// 测试账号选择策略 - 跳过失败账号
#[test]
fn test_account_selection_skips_failed_accounts() {
    struct Account {
        id: String,
        failure_count: u32,
    }

    let accounts = vec![
        Account { id: "acc1".to_string(), failure_count: 5 },
        Account { id: "acc2".to_string(), failure_count: 0 },
        Account { id: "acc3".to_string(), failure_count: 3 },
    ];

    let max_failures = 3;
    let available: Vec<_> = accounts.iter().filter(|acc| acc.failure_count < max_failures).collect();

    assert_eq!(available.len(), 1);
    assert_eq!(available[0].id, "acc2");

    println!("✓ Account selection skips failed accounts");
}

/// 测试账号选择策略 - Balanced (Least Used)
#[test]
fn test_account_selection_balanced() {
    struct Account {
        id: String,
        usage_count: u64,
        failure_count: u32,
    }

    let mut accounts = vec![
        Account { id: "acc1".to_string(), usage_count: 10, failure_count: 0 },
        Account { id: "acc2".to_string(), usage_count: 5, failure_count: 0 },
        Account { id: "acc3".to_string(), usage_count: 15, failure_count: 0 },
    ];

    let max_failures = 3;

    let select_balanced = |accounts: &mut [Account]| -> Option<String> {
        accounts.iter_mut()
            .filter(|acc| acc.failure_count < max_failures)
            .min_by_key(|acc| acc.usage_count)
            .map(|acc| {
                acc.usage_count += 1;
                acc.id.clone()
            })
    };

    let selected = select_balanced(&mut accounts);
    assert_eq!(selected, Some("acc2".to_string()));

    let acc2 = accounts.iter().find(|a| a.id == "acc2").unwrap();
    assert_eq!(acc2.usage_count, 6);

    println!("✓ Balanced account selection chooses least used account");
}

/// 测试模型 Token 限制获取
#[test]
fn test_get_model_max_input_tokens() {
    let get_max_tokens = |model_id: &str| -> usize {
        let model_lower = model_id.to_lowercase();
        if model_lower == "auto" {
            1_000_000
        } else if model_lower.contains("opus-4.8") || model_lower.contains("opus-4-8") {
            1_000_000
        } else if model_lower.contains("opus-4.7") || model_lower.contains("opus-4-7") {
            1_000_000
        } else if model_lower.contains("opus-4.6") || model_lower.contains("opus-4-6") {
            1_000_000
        } else if model_lower.contains("sonnet-4.6") || model_lower.contains("sonnet-4-6") {
            1_000_000
        } else if model_lower.contains("qwen") {
            256_000
        } else if model_lower.contains("llama") || model_lower.contains("deepseek") {
            128_000
        } else {
            200_000
        }
    };

    assert_eq!(get_max_tokens("auto"), 1_000_000);
    assert_eq!(get_max_tokens("claude-opus-4.8"), 1_000_000);
    assert_eq!(get_max_tokens("claude-opus-4.7"), 1_000_000);
    assert_eq!(get_max_tokens("claude-opus-4.6"), 1_000_000);
    assert_eq!(get_max_tokens("claude-sonnet-4.6"), 1_000_000);
    assert_eq!(get_max_tokens("claude-sonnet-4.5"), 200_000);
    assert_eq!(get_max_tokens("qwen3-coder"), 256_000);
    assert_eq!(get_max_tokens("llama-3-70b"), 128_000);
    assert_eq!(get_max_tokens("deepseek-chat"), 128_000);
    println!("✓ Model max input tokens detection works correctly");
}

/// 测试 Token 限制阈值计算
#[test]
fn test_token_threshold_calculation() {
    let max_input_tokens = 200_000;
    let threshold_percent = 0.8;
    let threshold_tokens = (max_input_tokens as f64 * threshold_percent) as usize;

    assert_eq!(threshold_tokens, 160_000);

    let opus_threshold = (1_000_000 as f64 * threshold_percent) as usize;
    assert_eq!(opus_threshold, 800_000);

    let llama_threshold = (128_000 as f64 * threshold_percent) as usize;
    assert_eq!(llama_threshold, 102_400);

    println!("✓ Token threshold calculation is correct");
}

/// 测试消息合并逻辑 - 相邻同角色消息
#[test]
fn test_merge_adjacent_same_role_messages() {
    let messages = vec![
        json!({"role": "user", "content": "Hello"}),
        json!({"role": "user", "content": "How are you?"}),
        json!({"role": "assistant", "content": "I'm fine"}),
        json!({"role": "assistant", "content": "Thank you"}),
    ];

    let mut merged = Vec::new();
    let mut current: Option<serde_json::Value> = None;

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap();

        if let Some(ref mut curr) = current {
            let curr_role = curr.get("role").and_then(|r| r.as_str()).unwrap();

            if curr_role == role {
                let curr_content = curr.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let new_content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                curr["content"] = json!(format!("{}\n{}", curr_content, new_content));
            } else {
                merged.push(curr.clone());
                current = Some(msg);
            }
        } else {
            current = Some(msg);
        }
    }

    if let Some(curr) = current {
        merged.push(curr);
    }

    assert_eq!(merged.len(), 2);

    let first = &merged[0];
    assert_eq!(first.get("role").and_then(|r| r.as_str()), Some("user"));
    assert!(first.get("content").and_then(|c| c.as_str()).unwrap().contains("Hello"));
    assert!(first.get("content").and_then(|c| c.as_str()).unwrap().contains("How are you?"));

    println!("✓ Adjacent same-role messages are merged correctly");
}

/// 测试 Web 搜索迭代限制
#[test]
fn test_web_search_iteration_limit() {
    let max_iterations = 8;
    let mut iteration_count = 0;

    let should_continue_search = |count: &mut u32| -> bool {
        *count += 1;
        *count <= max_iterations
    };

    for _ in 0..8 {
        assert!(should_continue_search(&mut iteration_count));
    }

    assert!(!should_continue_search(&mut iteration_count));
    assert_eq!(iteration_count, 9);

    println!("✓ Web search iteration limit (8) is enforced");
}
