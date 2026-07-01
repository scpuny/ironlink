// ── Codex tool context ──

use std::collections::BTreeMap;
use serde_json::Value;

#[derive(Debug, Clone)]
struct CustomToolSpec {
    openai_name: String,
    built_in: bool,
}

#[derive(Debug, Clone, Default)]
/// Tracks Codex custom tool mappings and namespace tools during SSE conversion.
pub struct CodexToolContext {
    custom_tools: BTreeMap<String, CustomToolSpec>,
    function_tools: BTreeMap<String, FunctionToolSpec>,
    pub has_custom_tools: bool,
    pub has_namespace_tools: bool,
}

#[derive(Debug, Clone, Default)]
struct FunctionToolSpec {
    pub namespace: String,
    pub name: String,
}

impl CodexToolContext {
    /// Build tool context from a Responses API tools array.
    pub fn from_request(tools: Option<&Value>) -> Self {
        let mut ctx = Self::default();
        let Some(tools) = tools.and_then(Value::as_array) else { return ctx; };

        for tool in tools {
            if let Some(name) = tool.as_str().filter(|n| !n.is_empty()) {
                ctx.custom_tools.insert(name.to_string(), CustomToolSpec { openai_name: name.to_string(), built_in: false });
                ctx.has_custom_tools = true;
                continue;
            }
            let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or("");
            match tool_type {
                "function" => {
                    if let Some(name) = tool.get("name").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                        ctx.function_tools.insert(name.to_string(), FunctionToolSpec { name: name.to_string(), namespace: String::new() });
                    }
                }
                "custom" | "web_search" | "local_shell" | "computer_use" => {
                    if let Some(name) = tool.get("name").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                        let built_in = matches!(tool_type, "web_search" | "local_shell" | "computer_use");
                        ctx.custom_tools.insert(name.to_string(), CustomToolSpec { openai_name: name.to_string(), built_in });
                        ctx.has_custom_tools = true;
                    }
                }
                _ => {}
            }
        }
        ctx
    }

    /// Check if a tool name corresponds to a registered custom tool.
    pub fn is_custom_tool_proxy(&self, name: &str) -> bool {
        self.custom_tools.contains_key(name)
    }

    /// Return the original custom tool name as declared by Codex.
    pub fn original_custom_tool_name(&self, name: &str) -> String {
        self.custom_tools.get(name).map(|s| s.openai_name.clone()).unwrap_or_else(|| name.to_string())
    }
}
