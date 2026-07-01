// ── Canonical protocol types ──

use serde_json::Value;

// ── Content ──

/// A single piece of content within a message.
#[derive(Debug, Clone)]
/// A single piece of content within a protocol message.
pub enum ContentPart {
    Text(String),
    Image { url: String, detail: Option<String> },
    File { data: String, filename: String },
    Refusal(String),
    Thinking(String),
    InputAudio { data: String, format: String },
}

// ── Roles ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The role of a message participant.
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

// ── Messages ──

#[derive(Debug, Clone)]
/// A single message in the canonical protocol representation.
pub struct ProtocolMessage {
    pub role: MessageRole,
    pub content: Vec<ContentPart>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

// ── Tools ──

#[derive(Debug, Clone)]
/// Classification of Codex-specific custom tool types.
pub enum CustomToolKind {
    Raw,
    ApplyPatch,
    BuiltIn,
    ApplyPatchAction(String), // e.g. "add_file", "batch"
}

#[derive(Debug, Clone)]
/// The type of a tool definition.
pub enum ToolType {
    Function,
    Custom(CustomToolKind),
    WebSearch,
    LocalShell,
    ComputerUse,
    Namespace(Vec<ToolDefinition>),
}

/// A tool definition (schema).
#[derive(Debug, Clone)]
/// A tool definition/schema in canonical form.
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
    pub tool_type: ToolType,
    pub strict: Option<bool>,
}

/// A tool call instance within a message.
#[derive(Debug, Clone)]
/// A tool call instance within a message.
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub tool_type: ToolType,
}

// ── Reasoning ──

#[derive(Debug, Clone)]
/// Reasoning configuration for a request.
pub struct ReasoningConfig {
    pub enabled: bool,
    pub effort: Option<String>,
}

// ── Request ──

/// Universal request that all protocols can be parsed into / built from.
#[derive(Debug, Clone)]
/// Universal canonical request that all protocols convert to/from.
pub struct ProtocolRequest {
    pub model: String,
    pub messages: Vec<ProtocolMessage>,
    pub system: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: Option<Value>,
    pub reasoning: Option<ReasoningConfig>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stream: bool,
    pub stream_options: Option<Value>,
    pub metadata: Option<Value>,
    pub extra_fields: Vec<(String, Value)>,
}

// ── Response ──

#[derive(Debug, Clone)]
/// An item in a canonical protocol response.
pub enum OutputItem {
    Message {
        role: String,
        content: Vec<ContentPart>,
    },
    Reasoning {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
        tool_type: ToolType,
    },
    CustomToolCall {
        id: String,
        name: String,
        input: String,
    },
}

#[derive(Debug, Clone)]
/// Token usage statistics in canonical form.
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: Option<u64>,
    pub extra: Vec<(String, Value)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The status of a response.
pub enum ResponseStatus {
    InProgress,
    Completed,
    Incomplete,
    Failed,
}

/// Universal response that all protocols can be built from / parsed into.
#[derive(Debug, Clone)]
/// Universal canonical response that all protocols convert to/from.
pub struct ProtocolResponse {
    pub id: String,
    pub model: String,
    pub created_at: u64,
    pub status: ResponseStatus,
    pub output: Vec<OutputItem>,
    pub usage: Usage,
    pub extra_fields: Vec<(String, Value)>,
}

// ── SSE Event ──

/// A parsed SSE event (protocol-agnostic).
#[derive(Debug, Clone)]
/// A parsed SSE event in protocol-agnostic form.
pub struct SseEvent {
    pub event: String,
    pub data: String,
}
