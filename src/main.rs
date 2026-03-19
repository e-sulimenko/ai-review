use anyhow::Result;

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
  let file_diffs = git::get_diff(&branch)?;
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

  // Отправляем файлы в LLM (MVP — фейковый ревью)
  let reviews = llm::review_files(&config.llm, &file_diffs, &logger).await?;
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
