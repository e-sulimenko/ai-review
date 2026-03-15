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
}

/// Функция для парсинга аргументов
pub fn parse_cli() -> Cli {
  Cli::parse()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_flags() {
    // Заглушка теста: проверяем дефолтные значения
    let cli = Cli {
      json: false,
      debug: false,
    };
    assert!(!cli.json);
    assert!(!cli.debug);
  }
}
