use anyhow::Result;

mod cli;
mod config;
mod git;
mod llm;
mod output;
mod review;

#[tokio::main]
async fn main() -> Result<()> {
  // Парсим CLI через отдельный модуль
  let cli = cli::parse_cli();

  // Загружаем конфиг
  let config = config::load_config()?;

  let branch = git::current_branch()?;

  // Получаем diff по текущей ветке (сравнение с HEAD рабочей копии)
  let file_diffs = git::get_diff(&branch)?;
  if file_diffs.is_empty() {
    println!("No changes detected relative to {}.", &branch);
    return Ok(());
  }

  // Отправляем файлы в LLM (MVP — фейковый ревью)
  let reviews = llm::review_files(&config.llm, &file_diffs).await?;
  println!("{:?}", reviews);
  // Агрегируем ревью
  let summary = review::aggregate_reviews(&file_diffs, reviews);

  println!("{:?}", summary);
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
