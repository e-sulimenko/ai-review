use crate::git::FileDiff;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum IssueSeverity {
  Error,
  Warning,
  Suggestion,
}

impl fmt::Display for IssueSeverity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      IssueSeverity::Error => write!(f, "error"),
      IssueSeverity::Warning => write!(f, "warning"),
      IssueSeverity::Suggestion => write!(f, "suggestion"),
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum IssueType {
  Bug,
  Style,
  Performance,
  Security,
}

impl fmt::Display for IssueType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      IssueType::Bug => write!(f, "bug"),
      IssueType::Style => write!(f, "style"),
      IssueType::Performance => write!(f, "performance"),
      IssueType::Security => write!(f, "security"),
    }
  }
}

/// Структура одного issue, возвращаемого LLM
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Issue {
  pub line: usize,
  pub severity: IssueSeverity,
  pub issue_type: IssueType,
  pub message: String,
  pub suggestion: String,
}

/// Ревью одного файла
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReview {
  pub path: String,
  pub issues: Vec<Issue>,
}

/// Отправка одного файла в LLM и получение ревью
/// На стадии MVP — заглушка с фейковым результатом
pub async fn review_file(file: &FileDiff) -> Result<FileReview> {
  // TODO: реальная интеграция с LLM через HTTP или SDK
  // Для MVP создаем фейковое ревью
  let fake_issues = if file.diff.contains("fn main") {
    vec![Issue {
      line: 1,
      severity: IssueSeverity::Warning,
      issue_type: IssueType::Style,
      message: "Consider adding documentation for main function".to_string(),
      suggestion: "Add /// comments above main".to_string(),
    }]
  } else {
    vec![]
  };

  Ok(FileReview {
    path: file.path.clone(),
    issues: fake_issues,
  })
}

/// Обработка всех файлов (async, параллельно)
pub async fn review_files(files: &[FileDiff]) -> Result<Vec<FileReview>> {
  use futures::stream::{self, StreamExt};

  let reviews = stream::iter(files)
    .map(|file| async move { review_file(file).await })
    .buffer_unordered(5) // одновременно обрабатываем до 5 файлов
    .collect::<Vec<_>>()
    .await;

  // Собираем результаты, проверяем ошибки
  let mut results = Vec::new();
  for r in reviews {
    results.push(r?);
  }

  Ok(results)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::git::FileDiff;

  #[tokio::test]
  async fn test_review_file() {
    let file = FileDiff {
      path: "src/main.rs".to_string(),
      diff: "fn main() {}".to_string(),
    };
    let review = review_file(&file).await.unwrap();
    assert_eq!(review.path, "src/main.rs");
    assert_eq!(review.issues.len(), 1);
    assert_eq!(review.issues[0].severity, IssueSeverity::Warning);
  }

  #[tokio::test]
  async fn test_review_files() {
    let files = vec![
      FileDiff {
        path: "src/main.rs".to_string(),
        diff: "fn main() {}".to_string(),
      },
      FileDiff {
        path: "src/lib.rs".to_string(),
        diff: "".to_string(),
      },
    ];
    let reviews = review_files(&files).await.unwrap();
    assert_eq!(reviews.len(), 2);
    assert_eq!(reviews[0].issues.len(), 1);
    assert_eq!(reviews[1].issues.len(), 0);
  }
}
