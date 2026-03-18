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

  let branch = match &cli.branch {
    Some(b) => b.clone(),
    None => git::current_branch()?,
  };

  // Получаем diff по default (origin/main)
  let file_diffs = git::get_diff(&branch, cli.fetch)?;
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
  } else if cli.debug {
    output::print_debug(&summary, &file_diffs);
  } else {
    output::print_readable(&summary);
  }

  Ok(())
}
