// ── Relay profile persistence ──

use std::path::PathBuf;
use crate::models::RelayProfile;

fn profiles_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("relay_profiles.json")
}

/// Read relay profiles from the JSON file on disk.
pub fn read() -> Vec<RelayProfile> {
    let path = profiles_path();
    match std::fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Persist relay profiles to the JSON file on disk.
pub fn write(profiles: &[RelayProfile]) -> anyhow::Result<()> {
    let path = profiles_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, serde_json::to_string_pretty(profiles)?)?;
    tracing::info!("relay_profiles.json written ({} profiles)", profiles.len());
    Ok(())
}
