use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub llm: LlmConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
}

pub fn load_config() -> Result<Config> {
    // 1. ~/.ai-review/config.json
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".ai-review/config.json");
        if path.exists() {
            return read_config(path);
        }
    }

    // 2. ./ai-review.json
    let local = PathBuf::from("ai-review.json");
    if local.exists() {
        return read_config(local);
    }

    // 3. ENV fallback
    let api_url = std::env::var("AI_REVIEW_API_URL")
        .context("Missing AI_REVIEW_API_URL")?;

    let api_key = std::env::var("AI_REVIEW_API_KEY")
        .context("Missing AI_REVIEW_API_KEY")?;

    let model = std::env::var("AI_REVIEW_MODEL")
        .unwrap_or_else(|_| "anthropic/claude-3.5-sonnet".to_string());

    Ok(Config {
        llm: LlmConfig {
            api_url,
            api_key,
            model,
        },
    })
}

fn read_config(path: PathBuf) -> Result<Config> {
    let content = fs::read_to_string(&path)
        .context(format!("Failed to read config {:?}", path))?;

    let config: Config = serde_json::from_str(&content)
        .context("Invalid config JSON")?;

    Ok(config)
}
