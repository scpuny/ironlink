//! Tool context structs for Responses ↔ Chat conversion.
//!
//! Models Codex's Responses API tool types directly as structs,
//! matching the approach used in CodexPlusPlus/cc-switch.
//! No abstract canonical intermediate types — just direct wire-format conversion.

use std::collections::BTreeMap;
use serde_json::Value;

// ── Tool kinds ──

/// Classification of Codex custom tool types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomToolKind {
    /// Unstructured input tool (default for string-name tools).
    Raw,
    /// The Codex `apply_patch` tool — gets split into multiple action tools.
    ApplyPatch,
    /// Built-in tool (web_search, local_shell, computer_use) passed as freeform.
    BuiltIn,
}

/// Proxy action for apply_patch sub-tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchProxyAction {
    AddFile,
    DeleteFile,
    UpdateFile,
    ReplaceFile,
    Batch,
}

impl PatchProxyAction {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::AddFile => "add_file",
            Self::DeleteFile => "delete_file",
            Self::UpdateFile => "update_file",
            Self::ReplaceFile => "replace_file",
            Self::Batch => "batch",
        }
    }
}

// ── Tool specifications ──

/// Metadata for a custom tool, tracking the original Responses API name.
#[derive(Debug, Clone)]
pub struct CustomToolSpec {
    /// The original tool name as known in the Responses API.
    pub responses_name: String,
    /// How this tool should be handled.
    pub kind: CustomToolKind,
    /// For apply_patch sub-tools, which action this represents.
    pub proxy_action: Option<PatchProxyAction>,
}

/// Metadata for a function tool, tracking namespace information.
#[derive(Debug, Clone)]
pub struct FunctionToolSpec {
    /// The original namespace (empty for non-namespace tools).
    pub namespace: String,
    /// The original tool name within the namespace.
    pub name: String,
}

// ── Tool context ──

/// Tracks all tool definitions from a Responses API request.
///
/// This context is used when converting:
/// 1. Request: Responses tools → Chat function tools (with name flattening)
/// 2. Response: Chat tool_calls → Responses output items (with name restoration)
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// Custom tools (string tools, custom type, web_search, etc.)
    pub custom_tools: BTreeMap<String, CustomToolSpec>,
    /// Function tools (with optional namespace)
    pub function_tools: BTreeMap<String, FunctionToolSpec>,
    /// Whether any custom tools were declared.
    pub has_custom_tools: bool,
    /// Whether any namespace tools were declared.
    pub has_namespace_tools: bool,
}

impl ToolContext {
    /// Check if a Chat function tool name maps to a custom tool.
    pub fn is_custom_proxy(&self, chat_name: &str) -> bool {
        self.custom_tools.contains_key(chat_name)
    }

    /// Get the original Responses API name for a Chat function tool name.
    pub fn original_responses_name(&self, chat_name: &str) -> String {
        self.custom_tools
            .get(chat_name)
            .map(|spec| spec.responses_name.clone())
            .unwrap_or_else(|| chat_name.to_string())
    }

    /// Look up the original function tool name and namespace from a Chat name.
    pub fn function_name_for_chat(&self, chat_name: &str) -> (String, String) {
        let Some(spec) = self.function_tools.get(chat_name) else {
            return (chat_name.to_string(), String::new());
        };
        let name = if spec.name.is_empty() {
            chat_name.to_string()
        } else {
            spec.name.clone()
        };
        (name, spec.namespace.clone())
    }

    /// Build context from a Responses API request body's tools array.
    pub fn from_request(tools: Option<&Value>) -> Self {
        let mut ctx = Self::default();
        let Some(arr) = tools.and_then(Value::as_array) else {
            return ctx;
        };

        for tool in arr {
            if let Some(name) = tool.as_str().filter(|n| !n.is_empty()) {
                if let Some(action) = proxy_action_from_name(name) {
                    ctx.custom_tools.insert(
                        name.to_string(),
                        CustomToolSpec {
                            responses_name: "apply_patch".to_string(),
                            kind: CustomToolKind::ApplyPatch,
                            proxy_action: Some(action),
                        },
                    );
                    ctx.has_custom_tools = true;
                    continue;
                }
                ctx.custom_tools.insert(
                    name.to_string(),
                    CustomToolSpec {
                        responses_name: name.to_string(),
                        kind: CustomToolKind::Raw,
                        proxy_action: None,
                    },
                );
                ctx.has_custom_tools = true;
                continue;
            }

            let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or("");
            match tool_type {
                "custom" => {
                    let Some(name) = tool.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) else {
                        continue;
                    };
                    let kind = detect_custom_kind(tool, name);
                    ctx.custom_tools.insert(
                        name.to_string(),
                        CustomToolSpec {
                            responses_name: name.to_string(),
                            kind,
                            proxy_action: None,
                        },
                    );
                    // For apply_patch, also register action sub-tools
                    if kind == CustomToolKind::ApplyPatch {
                        for action in [
                            PatchProxyAction::AddFile,
                            PatchProxyAction::DeleteFile,
                            PatchProxyAction::UpdateFile,
                            PatchProxyAction::ReplaceFile,
                            PatchProxyAction::Batch,
                        ] {
                            let proxy_name = format!("{}_{}", name, action.suffix());
                            ctx.custom_tools.insert(
                                proxy_name,
                                CustomToolSpec {
                                    responses_name: name.to_string(),
                                    kind: CustomToolKind::ApplyPatch,
                                    proxy_action: Some(action),
                                },
                            );
                        }
                    }
                    ctx.has_custom_tools = true;
                }
                "function" => {
                    // Support both flat format {type:"function", name:"x"} and
                    // nested format {type:"function", function:{name:"x"}}
                    let name = tool.get("name").and_then(Value::as_str)
                        .or_else(|| tool.pointer("/function/name").and_then(Value::as_str))
                        .filter(|n| !n.is_empty());
                    if let Some(name) = name {
                        ctx.function_tools.insert(
                            name.to_string(),
                            FunctionToolSpec {
                                name: name.to_string(),
                                namespace: String::new(),
                            },
                        );
                    }
                }
                "namespace" => {
                    add_namespace_tools(&mut ctx, tool);
                }
                "web_search" | "local_shell" | "computer_use" => {
                    let name = tool.get("name").and_then(Value::as_str).filter(|n| !n.is_empty())
                        .unwrap_or(tool_type).to_string();
                    ctx.custom_tools.insert(
                        name.clone(),
                        CustomToolSpec {
                            responses_name: name,
                            kind: CustomToolKind::BuiltIn,
                            proxy_action: None,
                        },
                    );
                    ctx.has_custom_tools = true;
                }
                _ => {}
            }
        }

        ctx
    }
}

// ── Helper: build namespace tool entries ──

fn add_namespace_tools(ctx: &mut ToolContext, namespace_tool: &Value) {
    let namespace = namespace_tool.get("name").and_then(Value::as_str).unwrap_or("");
    let Some(children) = namespace_tool.get("tools").and_then(Value::as_array) else {
        return;
    };
    for child in children {
        let tool_type = child.get("type").and_then(Value::as_str).unwrap_or("");
        let Some(name) = child.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) else {
            continue;
        };
        let flat = flatten_namespace_name(namespace, name);

        match tool_type {
            "function" => {
                if ctx.function_tools.get(&flat).is_none_or(|spec| !spec.namespace.is_empty()) {
                    ctx.function_tools.insert(
                        flat,
                        FunctionToolSpec {
                            namespace: namespace.to_string(),
                            name: name.to_string(),
                        },
                    );
                    ctx.has_namespace_tools = true;
                }
            }
            "custom" | "web_search" | "local_shell" | "computer_use" => {
                // Namespace-inner custom/built-in tools (e.g. CodeGraph tools)
                let kind = detect_custom_kind(child, name);
                ctx.custom_tools.insert(
                    flat,
                    CustomToolSpec {
                        responses_name: name.to_string(),
                        kind,
                        proxy_action: None,
                    },
                );
                ctx.has_custom_tools = true;
                ctx.has_namespace_tools = true;
            }
            _ => {} // skip unknown
        }
    }
}

// ── Helper: detect custom tool kind ──

fn detect_custom_kind(tool: &Value, name: &str) -> CustomToolKind {
    if name == "apply_patch" {
        return CustomToolKind::ApplyPatch;
    }
    if let Some(def) = tool.pointer("/format/definition").and_then(Value::as_str) {
        if def.contains("begin_patch") && def.contains("end_patch") && def.contains("add_hunk") {
            return CustomToolKind::ApplyPatch;
        }
    }
    CustomToolKind::Raw
}

// ── Helper: proxy action from name ──

fn proxy_action_from_name(name: &str) -> Option<PatchProxyAction> {
    match name {
        n if n.ends_with("_add_file") => Some(PatchProxyAction::AddFile),
        n if n.ends_with("_delete_file") => Some(PatchProxyAction::DeleteFile),
        n if n.ends_with("_update_file") => Some(PatchProxyAction::UpdateFile),
        n if n.ends_with("_replace_file") => Some(PatchProxyAction::ReplaceFile),
        n if n.ends_with("_batch") => Some(PatchProxyAction::Batch),
        _ => None,
    }
}

// ── Helper: flatten namespace tool name ──

/// Format: `namespace_name` (underscore separator)
pub fn flatten_namespace_name(namespace: &str, name: &str) -> String {
    if namespace.is_empty() { return name.to_string(); }
    format!("{namespace}__{name}")
}

// ── Helper: build chat function tool for custom tools ──

/// Build a generic custom proxy tool in Chat Completions format.
pub fn build_custom_proxy_tool(name: &str, description: &str) -> Value {
    // [FIX #13] Match CodexPlusPlus generic_custom_proxy_tool:
    // - Empty description → FREEFORM annotation
    // - Non-empty description → preserve + add FREEFORM note
    let tool_description = if description.trim().is_empty() {
        format!("FREEFORM custom tool: {name}. Put only the tool input text here.")
    } else {
        format!(
            "{}

This is a FREEFORM tool. Do not wrap the input in JSON or markdown.",
            description.trim()
        )
    };
    serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": tool_description,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Raw freeform input for this custom tool."
                    }
                },
                "required": ["input"]
            }
        }
    })
}

/// Build apply_patch proxy sub-tools.
pub fn build_apply_patch_proxy_tools(tool_name: &str, _description: &str) -> Vec<Value> {
    let mut tools = Vec::new();
    let apply_patch_desc = format!(
        "FREEFORM custom tool: {tool_name}. Put only the tool input text here."
    );
    for action in [
        PatchProxyAction::AddFile,
        PatchProxyAction::DeleteFile,
        PatchProxyAction::UpdateFile,
        PatchProxyAction::ReplaceFile,
        PatchProxyAction::Batch,
    ] {
        let proxy_name = format!("{tool_name}_{}", action.suffix());
        tools.push(build_custom_proxy_tool(&proxy_name, &apply_patch_desc));
    }
    tools
}
