use clap::Parser;

/// CLI аргументы
#[derive(Parser, Debug)]
#[command(
  author = "AI Reviewer",
  version = "0.1.0",
  about = "MVP AI Code Review CLI"
)]
pub struct Cli {
  /// Вывод в JSON
  #[arg(long)]
  pub json: bool,
  /// Сгенерировать Markdown отчет в `.ai-review/reviews`
  #[arg(long)]
  pub md: bool,
}

/// Функция для парсинга аргументов
pub fn parse_cli() -> Cli {
  Cli::parse()
}
