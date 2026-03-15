use anyhow::Result;

mod git;
mod llm;
mod review;
mod output;
mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Парсим CLI через отдельный модуль
    let cli = cli::parse_cli();

    // 1. Получаем diff по default (origin/main)
    let file_diffs = git::get_diff()?;
    if file_diffs.is_empty() {
        println!("No changes detected relative to origin/main.");
        return Ok(());
    }

    // 2. Отправляем файлы в LLM (MVP — фейковый ревью)
    let reviews = llm::review_files(&file_diffs).await?;

    // 3. Агрегируем ревью
    let summary = review::aggregate_reviews(&file_diffs, reviews);

    // 4. Выводим результаты
    if cli.json {
        output::print_json(&summary)?;
    } else if cli.debug {
        output::print_debug(&summary, &file_diffs);
    } else {
        output::print_readable(&summary);
    }

    Ok(())
}
