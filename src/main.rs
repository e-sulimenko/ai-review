use anyhow::Result;
use cli::{Command, InitArgs, RunArgs};

mod cache;
mod cli;
mod config;
mod init;
mod git;
mod llm;
mod output;
mod review;
mod ui_log;

#[tokio::main]
async fn main() -> Result<()> {
  let cli = cli::parse_cli();

  match cli.command {
    Command::CleanCache => {
      let cache_root = cache::default_cache_dir();
      let existed = cache_root.exists();
      cache::clear_cache_dir(cache_root)?;
      if existed {
        eprintln!("Review cache cleared ({}).", cache_root.display());
      } else {
        eprintln!(
          "No review cache at {} (nothing to remove).",
          cache_root.display()
        );
      }
      Ok(())
    }
    Command::CleanReview => {
      let reviews_root = output::default_reviews_dir();
      let existed = reviews_root.exists();
      output::clear_reviews_dir(reviews_root)?;
      if existed {
        eprintln!("Review reports cleared ({}).", reviews_root.display());
      } else {
        eprintln!(
          "No review reports at {} (nothing to remove).",
          reviews_root.display()
        );
      }
      Ok(())
    }
    Command::Clean => {
      let cache_root = cache::default_cache_dir();
      let reviews_root = output::default_reviews_dir();
      let cache_existed = cache_root.exists();
      let reviews_existed = reviews_root.exists();
      cache::clear_cache_dir(cache_root)?;
      output::clear_reviews_dir(reviews_root)?;
      match (cache_existed, reviews_existed) {
        (true, true) => eprintln!(
          "Cleared review cache ({}) and reports ({}).",
          cache_root.display(),
          reviews_root.display()
        ),
        (true, false) => eprintln!(
          "Cleared review cache ({}). No reports at {}.",
          cache_root.display(),
          reviews_root.display()
        ),
        (false, true) => eprintln!(
          "Cleared reports ({}). No cache at {}.",
          reviews_root.display(),
          cache_root.display()
        ),
        (false, false) => eprintln!(
          "Nothing to remove: neither {} nor {} exists.",
          cache_root.display(),
          reviews_root.display()
        ),
      }
      Ok(())
    }
    Command::Init(InitArgs { yes, global }) => init::run_init(yes, global),
    Command::Run(run_args) => run_review(run_args).await,
  }
}

async fn run_review(cli: RunArgs) -> Result<()> {
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
  let mut file_diffs = git::get_diff(&branch)?;

  if let Some(include) = &config.include {
    file_diffs.retain(|fd| {
      include.iter().any(|entry| path_matches_entry(&fd.path, entry))
    });
    logger.info(&format!(
      "After include filter: {} file(s) to review.",
      file_diffs.len()
    ));
  }
  if let Some(exclude) = &config.exclude {
    file_diffs.retain(|fd| {
      !exclude.iter().any(|entry| path_matches_entry(&fd.path, entry))
    });
    logger.info(&format!(
      "After exclude filter: {} file(s) to review.",
      file_diffs.len()
    ));
  }

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

  // Отправляем файлы в LLM; кеш можно отключить через --no-cache.
  let cache_root = (!cli.no_cache).then_some(cache::default_cache_dir());
  if cli.no_cache {
    logger.info("Review cache disabled (--no-cache).");
  }
  let reviews = llm::review_files(&config.llm, &file_diffs, &logger, cache_root).await?;
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

fn path_matches_entry(path: &str, entry: &str) -> bool {
  // Семантика простая и предсказуемая:
  // - если `entry` задаёт файл, то матчимся по `path.ends_with(entry)`
  // - если `entry` задаёт папку, то матчимся по `path.starts_with(entry)`
  // - допускаем как `src`, так и `src/`
  let path_norm = path.trim_start_matches("./");
  let entry_norm = entry.trim().trim_end_matches('/').trim_start_matches("./");
  if entry_norm.is_empty() {
    return false;
  }

  let path_p = std::path::Path::new(path_norm);
  let entry_p = std::path::Path::new(entry_norm);
  path_p.starts_with(entry_p) || path_p.ends_with(entry_p)
}
