//! Request routing: app → model mapping → provider.
//!
//! Routing flow:
//!   1. Find the app that matches the incoming protocol (e.g. "responses" → Codex Desktop)
//!   2. Look up the app's model_mappings for the requested model
//!   3. If matched → get provider_id + upstream_model from the mapping
//!   4. If no mapping → fall through to legacy prefix/direct provider matching
//!   5. Route to the resolved provider

use crate::models::{AppConfig, RelayProfile, MappingTarget};

/// Select a provider and upstream model for the given incoming model+protocol.
///
/// Priority:
///   a) App's model_mappings (explicit mapping from app's perspective)
///   b) Direct model match on providers (legacy prefix + model matching)
///   c) First enabled provider
pub fn select_provider<'a>(
    apps: &'a [AppConfig],
    profiles: &'a [RelayProfile],
    incoming_model: &str,
    protocol: &str,
) -> Option<(&'a RelayProfile, Option<String>)> {
    // ① Find the app that speaks this protocol
    let app = apps.iter().find(|a| a.enabled && a.protocol == protocol);

    // ② If app found, look up its model_mappings
    if let Some(app) = app {
        if let Some(target) = app.model_mappings.get(incoming_model) {
            if let Some(provider) = profiles.iter().find(|p| p.enabled && p.provider_id == target.provider_id) {
                return Some((provider, Some(target.upstream_model.clone())));
            }
        }
    }

    // ③ Fallback: legacy direct provider matching
    let enabled: Vec<&RelayProfile> = profiles.iter().filter(|p| p.enabled).collect();
    if enabled.is_empty() { return None; }

    // Prefix match: model="deepseek/deepseek-chat"
    if let Some(slash) = incoming_model.find('/') {
        let prefix = &incoming_model[..slash];
        let rest = &incoming_model[slash + 1..];
        for p in &enabled {
            if p.provider_id == prefix { return Some((p, Some(rest.to_string()))); }
        }
    }

    if incoming_model.is_empty() {
        return enabled.into_iter().next().map(|p| (p, None));
    }

    // Model name direct match
    for p in &enabled {
        if p.model == incoming_model || p.model_list.contains(&incoming_model.to_string()) {
            return Some((p, None));
        }
    }

    // Fallback
    enabled.into_iter().next().map(|p| (p, None))
}

/// Build backend URL based on path and relay profile protocol.
pub fn profile_url(base: &str, path: &str, protocol: &str) -> String {
    let base = base.trim_end_matches('/');
    match (protocol, path) {
        ("anthropic", "responses") | ("anthropic", "chat/completions") => format!("{}/messages", base),
        ("responses", "responses") => format!("{}/responses", base),
        ("responses", "chat/completions") => format!("{}/responses", base),
        ("chatCompletions", "responses") => format!("{}/chat/completions", base),
        _ => format!("{}/{}", base, path),
    }
}
