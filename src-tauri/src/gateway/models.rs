use serde::{Deserialize, Serialize};

/// 最大思考预算 tokens
const MAX_BUDGET_TOKENS: i32 = 24576;

/// Thinking 配置
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Thinking {
    #[serde(rename = "type")]
    pub thinking_type: String,
    #[serde(
        default = "default_budget_tokens",
        deserialize_with = "deserialize_budget_tokens"
    )]
    pub budget_tokens: i32,
}

impl Thinking {
    /// 是否启用了 thinking（enabled 或 adaptive）
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.thinking_type == "enabled" || self.thinking_type == "adaptive"
    }
}

fn default_budget_tokens() -> i32 {
    20000
}

fn default_max_tokens() -> i32 {
    4096
}

fn deserialize_budget_tokens<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = i32::deserialize(deserializer)?;
    Ok(value.min(MAX_BUDGET_TOKENS))
}

fn default_stream() -> bool {
    true
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct NormalizedRequest {
    pub model: String,
    pub messages: Vec<NormalizedMessage>,
    #[serde(default = "default_stream")]
    pub stream: bool,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub previous_response_id: Option<String>,
    #[serde(default)]
    pub thinking: Option<Thinking>,
    /// 工具名称反向映射表：sanitized_name -> original_name
    /// 用于在响应中还原工具名称（camelCase -> snake_case）
    #[serde(skip)]
    pub tool_name_map: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMessage {
    pub role: String,
    pub content: Option<serde_json::Value>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroPayload {
    pub conversation_state: ConversationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_arn: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationState {
    pub chat_trigger_type: String,
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_continuation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_task_type: Option<String>,
    pub current_message: CurrentMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<HistoryItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customization_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentMessage {
    pub user_input_message: UserInputMessage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInputMessage {
    pub content: String,
    pub model_id: String,
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_point: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_cache_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<DocumentBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_input_message_context: Option<UserInputMessageContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_intent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageBlock {
    pub format: String,
    pub source: ImageSource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum ImageSource {
    Bytes {
        bytes: String,  // Base64 编码的图片数据
    },
    Other {
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentBlock {
    pub format: String,
    pub name: String,
    pub source: DocumentSource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum DocumentSource {
    Bytes {
        bytes: String,  // Base64 编码的字节数据
    },
    FileId {
        file_id: String,  // Files API 上传后的文件ID
    },
    Other {
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInputMessageContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_studio_context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<KiroToolResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<KiroTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_settings: Option<serde_json::Value>,
}

// Tool 是联合类型
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum KiroTool {
    CachePoint {
        #[serde(rename = "cachePoint")]
        cache_point: serde_json::Value,
    },
    ToolSpecification {
        #[serde(rename = "toolSpecification")]
        tool_specification: KiroToolSpec,
    },
    Other {
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: KiroInputSchema,
}

#[derive(Debug, Clone, Serialize)]
pub struct KiroInputSchema {
    pub json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroToolResult {
    pub content: Vec<KiroToolResultContent>,
    pub status: String,
    pub tool_use_id: String,
}

// ToolResultContentBlock 是联合类型
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum KiroToolResultContent {
    Text {
        text: String,
    },
    Json {
        json: serde_json::Value,
    },
    Other {
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

// AWS EventStream 响应事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AssistantResponseEvent {
    pub content: String,
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_continuation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ToolUseEvent {
    pub tool_use_id: String,
    pub name: String,
    pub input: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct MessageMetadataEvent {
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_continuation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct UsageInfo {
    pub input_tokens: i32,
    pub output_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,
}

// ============================================================================
// OpenAI Responses API 结构体
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAIResponsesRequest {
    pub model: String,
    pub input: Vec<NormalizedMessage>,
    #[serde(default)]
    pub stream: bool,
    pub max_output_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub previous_response_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct OpenAIResponsesResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub output: Vec<ResponseOutputItem>,
    pub usage: OpenAIChatUsage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum ResponseOutputItem {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: Vec<ResponseContent>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ResponseContent {
    #[serde(rename = "text")]
    Text { text: String },
}

// Responses API 流式事件
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum ResponsesStreamEvent {
    #[serde(rename = "response.created")]
    ResponseCreated {
        id: String,
        object: String,
        created: i64,
    },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        item_index: i32,
        item: ResponseOutputItem,
    },
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        item_index: i32,
        content_index: i32,
        delta: String,
    },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone { item_index: i32 },
    #[serde(rename = "response.completed")]
    ResponseCompleted { usage: OpenAIChatUsage },
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HistoryItem {
    User {
        #[serde(rename = "userInputMessage")]
        user_input_message: HistoryUserMessage,
    },
    Assistant {
        #[serde(rename = "assistantResponseMessage")]
        assistant_response_message: HistoryAssistantMessage,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUserMessage {
    pub content: String,
    pub model_id: String,
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_input_message_context: Option<UserInputMessageContext>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAssistantMessage {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_uses: Option<Vec<KiroToolUse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplementary_web_links: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followup_prompt: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_point: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroToolUse {
    pub name: String,
    pub input: serde_json::Value,
    pub tool_use_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default)]
    pub system: Option<serde_json::Value>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    #[allow(dead_code)]
    pub top_k: Option<i32>,
    pub stop_sequences: Option<Vec<String>>,
    pub tools: Option<Vec<AnthropicTool>>,
    pub tool_choice: Option<serde_json::Value>,
    pub thinking: Option<Thinking>,
    #[allow(dead_code)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub context_editing: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub mcp_servers: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub betas: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub cache_control: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(default)]
    pub stream: bool,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub tools: Option<Vec<OpenAITool>>,
    pub tool_choice: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(default)]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAIToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChatChoice>,
    pub usage: OpenAIUsage,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatChoice {
    pub index: i32,
    pub message: OpenAIChatResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatResponseMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIResponseToolCall>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIResponseToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAIToolCallFunction,
}

// OpenAI Chat Completions API 使用量统计
#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChatChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIChatUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatChunkChoice {
    pub index: i32,
    pub delta: OpenAIChatDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIDeltaToolCall>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIDeltaToolCall {
    pub index: i32,
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAIToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicTool {
    #[serde(default)]
    pub r#type: Option<String>,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub max_uses: Option<i32>,
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(default)]
    pub user_location: Option<serde_json::Value>,
    #[serde(default)]
    pub cache_control: Option<serde_json::Value>,
}


#[derive(Debug, Clone, Serialize)]
pub struct AnthropicMessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnthropicUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
}
