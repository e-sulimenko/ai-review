use clap::{Parser, Subcommand};

/// CLI аргументы
#[derive(Parser, Debug)]
#[command(
  author = "AI Reviewer",
  version = "0.1.0",
  about = "MVP AI Code Review CLI"
)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
  /// Создать config.json (интерактивно или шаблон с --yes)
  Init(InitArgs),
  /// Запустить ревью изменений в текущей ветке
  Run(RunArgs),
  /// Удалить каталог кеша ревью (`.ai-review/cache`)
  #[command(name = "clean-cache")]
  CleanCache,
  /// Удалить каталог markdown-отчётов (`.ai-review/reviews`)
  #[command(name = "clean-review")]
  CleanReview,
  /// Удалить и кеш, и каталог отчётов (`.ai-review/cache` и `.ai-review/reviews`)
  Clean,
}

/// Аргументы подкоманды `init`
#[derive(clap::Args, Debug)]
pub struct InitArgs {
  /// Без вопросов: записать шаблон с значениями по умолчанию (как у eslint --yes)
  #[arg(long)]
  pub yes: bool,
  /// Записать конфиг в домашнюю директорию (~/.ai-review/config.json)
  #[arg(long)]
  pub global: bool,
}

/// Аргументы подкоманды `run`
#[derive(clap::Args, Debug)]
pub struct RunArgs {
  /// Вывод в JSON
  #[arg(long)]
  pub json: bool,
  /// Сгенерировать Markdown отчет в `.ai-review/reviews`
  #[arg(long)]
  pub md: bool,
  /// Enable verbose debug logs (for developers / advanced users).
  #[arg(long)]
  pub debug: bool,
  /// Не использовать кеш ревью (не читать и не записывать).
  #[arg(long = "no-cache")]
  pub no_cache: bool,
}

/// Функция для парсинга аргументов
pub fn parse_cli() -> Cli {
  Cli::parse()
}
