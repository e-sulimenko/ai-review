use crate::git::FileDiff;
use crate::llm::FileReview;
use serde::{Deserialize, Serialize};

/// Сводка по всем файлам
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewSummary {
  pub total_lines: usize,
  pub issues: usize,
  pub lines_to_fix: usize,
  pub files: Vec<FileReview>,
}

/// Агрегируем ревью по всем файлам
pub fn aggregate_reviews(files: &[FileDiff], reviews: Vec<FileReview>) -> ReviewSummary {
  let total_lines = files.iter().map(|f| f.diff.lines().count()).sum();
  let issues = reviews.iter().map(|r| r.issues.len()).sum();
  let lines_to_fix = reviews
    .iter()
    .map(|r| {
      r.issues
        .iter()
        .map(|issue| issue.line)
        .collect::<std::collections::HashSet<_>>()
        .len()
    })
    .sum();

  ReviewSummary {
    total_lines,
    issues,
    lines_to_fix,
    files: reviews,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::git::FileDiff;
  use crate::llm::{FileReview, Issue, IssueSeverity, IssueType};

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

    let reviews = vec![
      FileReview {
        path: "src/main.rs".to_string(),
        issues: vec![Issue {
          line: 1,
          severity: IssueSeverity::Warning,
          issue_type: IssueType::Style,
          message: "Add doc".to_string(),
          suggestion: "Add /// doc".to_string(),
        }],
      },
      FileReview {
        path: "src/lib.rs".to_string(),
        issues: vec![],
      },
    ];

    let summary = aggregate_reviews(&files, reviews);
    assert_eq!(summary.total_lines, 2);
    assert_eq!(summary.issues, 1);
    assert_eq!(summary.lines_to_fix, 1);
    assert_eq!(summary.files.len(), 2);
  }
}
