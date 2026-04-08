use anyhow::Result;

mod cache;
mod cli;
mod config;
mod git;
mod llm;
mod output;
mod review;
mod ui_log;

#[tokio::main]
async fn main() -> Result<()> {
  // Парсим CLI через отдельный модуль
  let cli = cli::parse_cli();
  let logger = ui_log::UiLogger::new(cli.json, cli.debug);

  logger.info("Starting AI code review...");

  // Загружаем конфиг
  let config = config::load_config()?;
  logger.info("Configuration loaded.");
  if logger.debug_enabled() {
    logger.debug(&format!(
      "LLM config: api_url={}, model={}, max_retry_count={}, candidate_reviews_per_diff={}, extra_body_keys={:?}",
      config.llm.api_url,
      config.llm.model,
      config.llm.max_retry_count,
      config.llm.candidate_reviews_per_diff,
      config.llm.extra_body.keys().collect::<Vec<_>>()
    ));
  }

  logger.info("Checking git branch and changes...");
  let branch = git::current_branch()?;
  if logger.debug_enabled() {
    logger.debug(&format!("Current git branch: {}", branch));
  }

  // Получаем diff по текущей ветке (сравнение с HEAD рабочей копии)
  let mut file_diffs = git::get_diff(&branch)?;

  if let Some(include) = &config.include {
    file_diffs.retain(|fd| {
      include.iter().any(|entry| path_matches_entry(&fd.path, entry))
    });
    logger.info(&format!(
      "After include filter: {} file(s) to review.",
      file_diffs.len()
    ));
  }
  if let Some(exclude) = &config.exclude {
    file_diffs.retain(|fd| {
      !exclude.iter().any(|entry| path_matches_entry(&fd.path, entry))
    });
    logger.info(&format!(
      "After exclude filter: {} file(s) to review.",
      file_diffs.len()
    ));
  }

  if file_diffs.is_empty() {
    logger.info(&format!("No changes detected relative to {}.", &branch));
    return Ok(());
  }
  logger.info(&format!("Found {} file(s) to review.", file_diffs.len()));
  if logger.debug_enabled() {
    for (idx, fd) in file_diffs.iter().enumerate() {
      logger.debug(&format!(
        "[{}/{}] diff file: path={}, diff_chars={}, diff_lines={}",
        idx + 1,
        file_diffs.len(),
        fd.path,
        fd.diff.len(),
        fd.diff.lines().count()
      ));
    }
  }

  // Отправляем файлы в LLM; кеш можно отключить через --no-cache.
  let cache_root = (!cli.no_cache).then_some(cache::default_cache_dir());
  if cli.no_cache {
    logger.info("Review cache disabled (--no-cache).");
  }
  let reviews = llm::review_files(&config.llm, &file_diffs, &logger, cache_root).await?;
  // Агрегируем ревью
  let summary = review::aggregate_reviews(&file_diffs, reviews);

  logger.info("Review complete. Producing report...");
  // Выводим результаты
  if cli.json {
    output::print_json(&summary)?;
  } else if cli.md {
    output::write_md_report(&summary)?;
  } else {
    output::print_readable(&summary);
  }

  Ok(())
}

fn path_matches_entry(path: &str, entry: &str) -> bool {
  // Семантика простая и предсказуемая:
  // - если `entry` задаёт файл, то матчимся по `path.ends_with(entry)`
  // - если `entry` задаёт папку, то матчимся по `path.starts_with(entry)`
  // - допускаем как `src`, так и `src/`
  let path_norm = path.trim_start_matches("./");
  let entry_norm = entry.trim().trim_end_matches('/').trim_start_matches("./");
  if entry_norm.is_empty() {
    return false;
  }

  let path_p = std::path::Path::new(path_norm);
  let entry_p = std::path::Path::new(entry_norm);
  path_p.starts_with(entry_p) || path_p.ends_with(entry_p)
}
