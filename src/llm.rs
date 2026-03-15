use crate::{config::LlmConfig, git::FileDiff};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
  Performance,
  Security,
}

impl fmt::Display for IssueType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      IssueType::Bug => write!(f, "bug"),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmReview {
  pub issues: Vec<Issue>,
}

pub async fn review_files(config: &LlmConfig, diffs: &[FileDiff]) -> Result<LlmReview> {
  let max_concurrent = 5;

  let reviews: Vec<LlmReview> = stream::iter(diffs)
    .map(|file| review_single_file(config, file))
    .buffer_unordered(max_concurrent)
    .collect()
    .await;

  Ok(merge_reviews(reviews))
}

async fn review_single_file(config: &LlmConfig, file: &FileDiff) -> LlmReview {
  let client = Client::new();

  let prompt = build_prompt_single(file);

  let body = json!({
      "model": config.model,
      "messages": [
          {
              "role": "system",
              "content": "You are a senior engineer performing strict code review. Return JSON only."
          },
          {
              "role": "user",
              "content": prompt
          }
      ],
      "temperature": 0
  });

  let resp = client
    .post(&config.api_url)
    .bearer_auth(&config.api_key)
    .json(&body)
    .send()
    .await;

  if let Ok(resp) = resp {
    if let Ok(text) = resp.text().await {
      if let Ok(review) = serde_json::from_str::<LlmReview>(&text) {
        return review;
      }
    }
  }

  LlmReview { issues: vec![] }
}

fn build_prompt_single(file: &FileDiff) -> String {
  format!(
    r#"
  Perform a strict code review.
  
  Check for:
  - syntax errors
  - type errors
  - logical bugs
  
  Return STRICT JSON:
  
  {{
   "issues":[
     {{
       "file":"{}",
       "line":123,
       "message":"short description",
       "suggestion":"fix"
     }}
   ]
  }}
  
  FILE:
  {}
  
  "#,
    file.path, file.diff
  )
}

fn merge_reviews(reviews: Vec<LlmReview>) -> LlmReview {
  let mut issues = Vec::new();

  for review in reviews {
    issues.extend(review.issues);
  }

  LlmReview { issues }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::LlmConfig;
  use crate::git::FileDiff;

  #[tokio::test]
  async fn test_review_file() {
    let file = FileDiff {
      path: "src/main.rs".to_string(),
      diff: "fn main() {}".to_string(),
    };
    let config = test_llm_config();
    let review = review_single_file(&config, &file).await;
    assert_eq!(review.issues.len(), 1);
    assert_eq!(review.issues[0].severity, IssueSeverity::Warning);
  }

  fn test_llm_config() -> LlmConfig {
    LlmConfig {
      api_url: "http://test".to_string(),
      api_key: "test".to_string(),
      model: "test".to_string(),
    }
  }

  #[tokio::test]
  async fn test_review_files() {
    let config = test_llm_config();
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
    let review = review_files(&config, &files).await.unwrap();
    assert!(review.issues.len() <= 2);
  }
}
