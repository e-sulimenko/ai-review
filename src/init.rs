use anyhow::{bail, Context, Result};
use serde_json::json;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::{global_config_path, local_config_path};

const DEFAULT_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_API_KEY: &str = "YOUR_API_KEY";
const DEFAULT_MODEL: &str = "openrouter/auto";

pub fn run_init(yes: bool, global: bool) -> Result<()> {
  let path = resolve_target_path(global)?;

  if path.exists() && !yes {
    eprint!(
      "Config already exists at {}.\nOverwrite? [y/N]: ",
      path.display()
    );
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let ok = matches!(line.trim().to_lowercase().as_str(), "y" | "yes");
    if !ok {
      bail!("Cancelled.");
    }
  }

  let (api_url, api_key, model) = if yes {
    (
      DEFAULT_API_URL.to_string(),
      DEFAULT_API_KEY.to_string(),
      DEFAULT_MODEL.to_string(),
    )
  } else {
    (
      prompt_required("? API URL (OpenAI-compatible chat completions endpoint): ")?,
      prompt_required("? API key (stored as plain text in config): ")?,
      prompt_required("? Model id (e.g. provider/model-name): ")?,
    )
  };

  let value = json!({
    "llm": {
      "api_url": api_url,
      "api_key": api_key,
      "model": model,
    }
  });

  let json =
    serde_json::to_string_pretty(&value).context("serialize config")?;

  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).with_context(|| {
      format!("create config directory {}", parent.display())
    })?;
  }

  std::fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;

  eprintln!(
    "Wrote {} ({}, {})",
    std::fs::canonicalize(&path).unwrap_or(path.clone()).display(),
    if global { "global" } else { "local" },
    if yes { "template (--yes)" } else { "interactive" },
  );

  Ok(())
}

fn resolve_target_path(global: bool) -> Result<PathBuf> {
  if global {
    global_config_path().context("could not resolve home directory for --global")
  } else {
    Ok(local_config_path())
  }
}

fn prompt_required(prompt: &str) -> Result<String> {
  loop {
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin()
      .read_line(&mut line)
      .context("read stdin")?;
    let value = line.trim().to_string();
    if !value.is_empty() {
      return Ok(value);
    }
    eprintln!("  (value required, try again)");
  }
}
