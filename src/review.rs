use crate::git::FileDiff;
use crate::llm::{FileReview, LlmReview};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewError {
  pub file: String,
  pub reason: String,
}

/// Сводка по всем файлам
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewSummary {
  pub total_lines: usize,
  pub issues: usize,
  pub lines_to_fix: usize,
  pub files: Vec<FileReview>,
}

/// Агрегируем ревью по всем файлам (LlmReview — один объединённый результат)
pub fn aggregate_reviews(files: &[FileDiff], review: LlmReview) -> ReviewSummary {
  let total_lines = files.iter().map(|f| f.diff.lines().count()).sum();
  let issues = review.issues.len();
  let lines_to_fix = review
    .issues
    .iter()
    .map(|issue| issue.line)
    .collect::<std::collections::HashSet<_>>()
    .len();

  let files = vec![FileReview {
    path: "".to_string(),
    issues: review.issues,
  }];

  ReviewSummary {
    total_lines,
    issues,
    lines_to_fix,
    files,
  }
}
