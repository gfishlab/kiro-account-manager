use serde::{Deserialize, Serialize};

/// IDE Session 完整数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeSession {
    pub session_id: String,
    pub title: String,
    pub session_type: String,
    pub workspace_directory: String,
    pub history: Vec<HistoryItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_summary: Option<String>,
    // 忽略其他未知字段
}

/// Session 摘要（用于列表显示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub title: String,
    #[serde(rename = "sessionType")]
    pub session_type: String,
    #[serde(rename = "workspaceDirectory")]
    pub workspace_directory: String,
    #[serde(rename = "workspaceHash")]
    pub workspace_hash: String,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    #[serde(rename = "createdAt")]
    pub created_at: Option<i64>,
    #[serde(rename = "modifiedAt")]
    pub modified_at: Option<i64>,
}

/// 对话历史项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub message: Message,
    #[serde(default)]
    pub context_items: Vec<serde_json::Value>,
    #[serde(default)]
    pub editor_state: serde_json::Value,
    #[serde(default)]
    pub prompt_logs: Vec<PromptLog>,
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(with = "content_format")]
    pub content: Vec<ContentItem>,
    #[serde(rename = "isHidden", default)]
    pub is_hidden: bool,
    pub id: String,
}

/// 消息内容项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

// 自定义序列化/反序列化，支持字符串或数组格式
mod content_format {
    use super::ContentItem;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(content: &Vec<ContentItem>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::Serialize;
        content.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ContentItem>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        use serde_json::Value;

        let value = Value::deserialize(deserializer)?;
        
        match value {
            // 如果是字符串，转换为单个 ContentItem
            Value::String(s) => Ok(vec![ContentItem {
                content_type: "text".to_string(),
                text: s,
            }]),
            // 如果是数组，正常解析
            Value::Array(arr) => {
                let mut items = Vec::new();
                for item in arr {
                    if let Value::Object(obj) = item {
                        let content_type = obj.get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("text")
                            .to_string();
                        let text = obj.get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        items.push(ContentItem { content_type, text });
                    }
                }
                Ok(items)
            }
            _ => Err(Error::custom("content must be string or array")),
        }
    }
}

/// Prompt 日志
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLog {
    pub model_title: String,
    pub prompt: String,
    pub completion: String,
    #[serde(default)]
    pub completion_options: serde_json::Value,
}
