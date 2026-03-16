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
  /// Вывод debug (diff + issues)
  #[arg(long)]
  pub debug: bool,
  /// Ветка для сравнения (по умолчанию текущая ветка)
  #[arg(long)]
  pub branch: Option<String>,

  /// Обновлять ветку с удалённого репозитория перед diff
  #[arg(long)]
  pub fetch: bool,
}

/// Функция для парсинга аргументов
pub fn parse_cli() -> Cli {
  Cli::parse()
}
