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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::git::FileDiff;
  use crate::llm::{Issue, IssueSeverity, IssueType, LlmReview};

  #[test]
  fn test_aggregate_reviews() {
    let files = vec![
      FileDiff {
        path: "src/main.rs".to_string(),
        diff: "fn main() {}".to_string(),
      },
      FileDiff {
        path: "src/lib.rs".to_string(),
        diff: "pub fn test() {}".to_string(),
      },
    ];

    let review = LlmReview {
      issues: vec![Issue {
        line: 1,
        severity: IssueSeverity::Error,
        issue_type: IssueType::Bug,
        message: "Add doc".to_string(),
        suggestion: "Add /// doc".to_string(),
      }],
    };

    let summary = aggregate_reviews(&files, review);
    assert_eq!(summary.total_lines, 2);
    assert_eq!(summary.issues, 1);
    assert_eq!(summary.lines_to_fix, 1);
    assert_eq!(summary.files.len(), 1);
  }
}
