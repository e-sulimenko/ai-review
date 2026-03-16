use crate::review::ReviewSummary;
use anyhow::Result;
use serde_json;

/// Вывод в консоль человекочитаемого формата
pub fn print_readable(summary: &ReviewSummary) {
  println!("================ AI Code Review ================");
  println!("Files changed: {}", summary.files.len());
  println!("Lines changed: {}", summary.total_lines);
  println!("Issues found: {}", summary.issues);
  println!("Lines to fix: {}", summary.lines_to_fix);
  println!("================================================");

  for file in &summary.files {
    if file.issues.is_empty() {
      continue;
    }
    println!("\n{}", file.path);
    println!("--------------------");
    for issue in &file.issues {
      println!(
        "Line {} [{}] {}",
        issue.line,
        issue.severity.to_string(),
        issue.message
      );
      println!("Suggestion: {}", issue.suggestion);
    }
  }
}

/// Вывод JSON (для --json)
pub fn print_json(summary: &ReviewSummary) -> Result<()> {
  let json = serde_json::to_string_pretty(summary)?;
  println!("{}", json);
  Ok(())
}

/// Debug вывод — показывает diff + issues
pub fn print_debug(summary: &ReviewSummary, file_diffs: &[crate::git::FileDiff]) {
  println!("================ DEBUG AI Code Review ================");
  for file in file_diffs {
    println!("\nFile: {}", file.path);
    println!("--- Diff ---");
    println!("{}", file.diff);

    if let Some(review) = summary.files.iter().find(|r| r.path == file.path) {
      println!("--- Issues ---");
      if review.issues.is_empty() {
        println!("No issues");
      } else {
        for issue in &review.issues {
          println!(
            "Line {} [{}] {} -> {}",
            issue.line,
            issue.severity.to_string(),
            issue.message,
            issue.suggestion
          );
        }
      }
    }
  }
  println!("=====================================================");
}
