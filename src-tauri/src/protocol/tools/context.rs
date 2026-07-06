// ── Codex tool context ──

use std::collections::BTreeMap;
use serde_json::Value;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CustomToolSpec {
    openai_name: String,
    built_in: bool,
}

/// Sub-tool suffixes split from `apply_patch` for Chat Completions proxy.
const APPLY_PATCH_SUB_TOOLS: &[&str] = &[
    "add_file", "delete_file", "update_file", "replace_file", "batch",
];

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
                ctx.register_apply_patch_subtools(name);
                continue;
            }
            let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or("");
            match tool_type {
                "function" => {
                    // Support both flat format {type:"function", name:"x"} and
                    // nested format {type:"function", function:{name:"x"}}
                    let name_val = tool.get("name").and_then(Value::as_str)
                        .or_else(|| tool.pointer("/function/name").and_then(Value::as_str));
                    if let Some(name) = name_val.filter(|v| !v.is_empty()) {
                        ctx.function_tools.insert(name.to_string(), FunctionToolSpec { name: name.to_string(), namespace: String::new() });
                    }
                }
                "custom" | "web_search" | "local_shell" | "computer_use" => {
                    if let Some(name) = tool.get("name").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                        let built_in = matches!(tool_type, "web_search" | "local_shell" | "computer_use");
                        ctx.custom_tools.insert(name.to_string(), CustomToolSpec { openai_name: name.to_string(), built_in });
                        ctx.has_custom_tools = true;
                        ctx.register_apply_patch_subtools(name);
                    }
                }
                "namespace" => {
                    if let Some(children) = tool.get("tools").and_then(Value::as_array) {
                        let namespace = tool.get("name").and_then(Value::as_str).unwrap_or("");
                        for child in children {
                            let child_type = child.get("type").and_then(Value::as_str).unwrap_or("");
                            let Some(cname) = child.get("name").and_then(Value::as_str).filter(|v| !v.is_empty()) else {
                                continue;
                            };
                            let flat = format!("{}__{}", namespace, cname);
                            match child_type {
                                "function" => {
                                    ctx.function_tools.insert(flat, FunctionToolSpec { name: cname.to_string(), namespace: namespace.to_string() });
                                }
                                "custom" | "web_search" | "local_shell" | "computer_use" => {
                                    let built_in = matches!(child_type, "web_search" | "local_shell" | "computer_use");
                                    ctx.custom_tools.insert(flat, CustomToolSpec { openai_name: cname.to_string(), built_in });
                                    ctx.has_custom_tools = true;
                                }
                                _ => {}
                            }
                        }
                        ctx.has_namespace_tools = true;
                    }
                }
                _ => {}
            }
        }
        ctx
    }

    /// Register Chat Completions sub-tool names for `apply_patch` so the SSE
    /// converter recognises them as custom tool proxies.
    fn register_apply_patch_subtools(&mut self, name: &str) {
        if name != "apply_patch" {
            return;
        }
        for suffix in APPLY_PATCH_SUB_TOOLS {
            let proxy_name = format!("apply_patch_{suffix}");
            self.custom_tools.entry(proxy_name).or_insert_with(|| {
                self.has_custom_tools = true;
                CustomToolSpec { openai_name: "apply_patch".to_string(), built_in: false }
            });
        }
    }

    /// Check if a tool name corresponds to a registered custom tool.
    pub fn is_custom_tool_proxy(&self, name: &str) -> bool {
        self.custom_tools.contains_key(name)
    }

    /// Return the original custom tool name as declared by Codex.
    pub fn original_custom_tool_name(&self, name: &str) -> String {
        self.custom_tools.get(name).map(|s| s.openai_name.clone()).unwrap_or_else(|| name.to_string())
    }

    /// [FIX #48] Look up original function tool name and namespace from flat Chat name.
    /// Returns (original_name, namespace) — namespace is empty for non-namespace functions.
    pub fn original_function_tool_name(&self, flat_name: &str) -> (String, String) {
        if let Some(spec) = self.function_tools.get(flat_name) {
            let name = if spec.name.is_empty() {
                flat_name.to_string()
            } else {
                spec.name.clone()
            };
            (name, spec.namespace.clone())
        } else {
            (flat_name.to_string(), String::new())
        }
    }
}
