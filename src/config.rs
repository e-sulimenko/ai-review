use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use serde_json::{Map, Value};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
  pub llm: LlmConfig,
}

fn default_max_retry_count() -> usize {
  3
}

fn default_candidate_reviews_per_diff() -> usize {
  2
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
  pub api_url: String,
  pub api_key: String,
  pub model: String,
  /// Сколько раз пробовать запрос к LLM при ошибках парсинга/валидации JSON.
  #[serde(default = "default_max_retry_count")]
  pub max_retry_count: usize,

  /// Сколько "кандидатных" ревью нужно сгенерировать для одного diff-файла
  /// перед дедупликацией.
  #[serde(default = "default_candidate_reviews_per_diff")]
  pub candidate_reviews_per_diff: usize,
  /// Дополнительные поля, которые будут добавлены в JSON body
  /// запроса к LLM на том же уровне, что и `model`/`messages`.
  #[serde(default)]
  pub extra_body: Map<String, Value>,
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
  let local = PathBuf::from(".ai-review/config.json");
  if local.exists() {
    return read_config(local);
  }

  // 3. ENV fallback
  let api_url = std::env::var("AI_REVIEW_API_URL").context("Missing AI_REVIEW_API_URL")?;

  let api_key = std::env::var("AI_REVIEW_API_KEY").context("Missing AI_REVIEW_API_KEY")?;

  let model =
    std::env::var("AI_REVIEW_MODEL").unwrap_or_else(|_| "anthropic/claude-3.5-sonnet".to_string());

  Ok(Config {
    llm: LlmConfig {
      api_url,
      api_key,
      model,
      max_retry_count: default_max_retry_count(),
      candidate_reviews_per_diff: default_candidate_reviews_per_diff(),
      extra_body: Default::default(),
    },
  })
}

fn read_config(path: PathBuf) -> Result<Config> {
  let content = fs::read_to_string(&path).context(format!("Failed to read config {:?}", path))?;

  let config: Config = serde_json::from_str(&content).context("Invalid config JSON")?;

  Ok(config)
}
