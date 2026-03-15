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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::llm::{FileReview, Issue, IssueSeverity, IssueType};
  use crate::review::ReviewSummary;

  #[test]
  fn test_print_readable() {
    let summary = ReviewSummary {
      total_lines: 3,
      issues: 1,
      lines_to_fix: 1,
      files: vec![FileReview {
        path: "src/main.rs".to_string(),
        issues: vec![Issue {
          line: 1,
          severity: IssueSeverity::Warning,
          issue_type: IssueType::Style,
          message: "Add doc".to_string(),
          suggestion: "Add /// doc".to_string(),
        }],
      }],
    };
    print_readable(&summary); // просто проверка что не падает
  }

  #[test]
  fn test_print_json() {
    let summary = ReviewSummary {
      total_lines: 0,
      issues: 0,
      lines_to_fix: 0,
      files: vec![],
    };
    assert!(print_json(&summary).is_ok());
  }
}
