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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_flags() {
    // Заглушка теста: проверяем дефолтные значения
    let cli = Cli::parse_from(&["ai-review"]);
    assert!(!cli.json);
    assert!(!cli.debug);
    assert_eq!(cli.branch, None);
    assert!(!cli.fetch);
  }

  #[test]
  fn test_custom_branch() {
      let cli = Cli::parse_from(&["ai-review", "--branch", "develop"]);
      assert_eq!(cli.branch, Some("develop".to_string()));
      assert!(!cli.fetch);
  }

  #[test]
  fn test_fetch() {
      let cli = Cli::parse_from(&["ai-review", "--fetch"]);
      assert!(cli.fetch);
  }
}
