use crate::git::FileDiff;
use crate::llm::{FileReview, LlmReview};
use serde::{Deserialize, Serialize};

/// Сводка по всем файлам
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewSummary {
  pub total_lines: usize,
  pub issues: usize,
  pub lines_to_fix: usize,
  pub files: Vec<FileReview>,
}

/// Агрегируем ревью по всем файлам (LlmReview — один объединённый результат)
pub fn aggregate_reviews(files: &[FileDiff], review: Vec<LlmReview>) -> ReviewSummary {
  let total_lines = files.iter().map(|f| f.diff.lines().count()).sum();
  let all_issues: Vec<_> = review.iter().flat_map(|r| r.issues.iter()).collect();
  let issues = all_issues.len();
  let lines_to_fix = all_issues
    .iter()
    .map(|issue| issue.line)
    .collect::<std::collections::HashSet<_>>()
    .len();

  let files = review
    .into_iter()
    .map(|r| FileReview {
      path: r.path,
      issues: r.issues,
      errors: r.errors,
    })
    .collect();

  ReviewSummary {
    total_lines,
    issues,
    lines_to_fix,
    files,
  }
}
