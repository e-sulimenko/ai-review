use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
  pub llm: LlmConfig,
  #[serde(default)]
  pub include: Option<Vec<String>>,
  #[serde(default)]
  pub exclude: Option<Vec<String>>,
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

/// Глобальный конфиг в домашней директории (`~/.ai-review/config.json`) и локальный в cwd
/// (`.ai-review/config.json`) сливаются: локальные ключи перекрывают глобальные, вложенные объекты
/// объединяются рекурсивно (как `git config` global + local).
pub fn load_config() -> Result<Config> {
  let global_path = dirs::home_dir().map(|h| h.join(".ai-review/config.json"));
  let local_path = PathBuf::from(".ai-review/config.json");

  let global_val = global_path
    .as_ref()
    .filter(|p| p.exists())
    .map(|p| read_json_value(p))
    .transpose()?;

  let local_val = local_path
    .exists()
    .then(|| read_json_value(&local_path))
    .transpose()?;

  let merged = match (global_val, local_val) {
    (Some(g), Some(l)) => merge_json_values(g, l),
    (Some(g), None) => g,
    (None, Some(l)) => l,
    (None, None) => {
      let global_hint = global_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.ai-review/config.json".to_string());
      anyhow::bail!(
        "Конфиг не найден. Создайте файл `{global_hint}` в домашней директории \
         или `{}` в текущей директории.",
        local_path.display(),
      );
    }
  };

  let config: Config = serde_json::from_value(merged).context("Invalid config JSON")?;

  validate_config(&config)?;
  Ok(config)
}

fn read_json_value(path: &Path) -> Result<Value> {
  let content = fs::read_to_string(path).context(format!("Failed to read config {:?}", path))?;
  serde_json::from_str(&content).context("Invalid config JSON")
}

fn merge_json_values(base: Value, overlay: Value) -> Value {
  match (base, overlay) {
    (Value::Object(mut base_map), Value::Object(overlay_map)) => {
      for (k, v) in overlay_map {
        let merged = match base_map.remove(&k) {
          Some(bv) if bv.is_object() && v.is_object() => merge_json_values(bv, v),
          _ => v,
        };
        base_map.insert(k, merged);
      }
      Value::Object(base_map)
    }
    (_, overlay) => overlay,
  }
}

fn validate_config(config: &Config) -> Result<()> {
  if config.include.is_some() && config.exclude.is_some() {
    anyhow::bail!(
      "В конфиге должен быть указан только один параметр: либо `include`, либо `exclude`."
    );
  }
  Ok(())
}
