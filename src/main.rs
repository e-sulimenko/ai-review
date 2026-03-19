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
  let logger = ui_log::UiLogger::new(cli.json);

  logger.info("Starting AI code review...");

  // Загружаем конфиг
  let config = config::load_config()?;
  logger.info("Configuration loaded.");

  logger.info("Checking git branch and changes...");
  let branch = git::current_branch()?;

  // Получаем diff по текущей ветке (сравнение с HEAD рабочей копии)
  let file_diffs = git::get_diff(&branch)?;
  if file_diffs.is_empty() {
    logger.info(&format!("No changes detected relative to {}.", &branch));
    return Ok(());
  }
  logger.info(&format!("Found {} file(s) to review.", file_diffs.len()));

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
