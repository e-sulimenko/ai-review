use crate::{config::LlmConfig, git::FileDiff, review::ReviewError};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
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
#[serde(rename_all = "lowercase")]
pub enum IssueType {
  Syntax,
  Type,
  Logic,
  Style,
  Performance,
  Security,
}

impl fmt::Display for IssueType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      IssueType::Syntax => write!(f, "syntax"),
      IssueType::Type => write!(f, "type"),
      IssueType::Logic => write!(f, "logic"),
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
  pub file: String,
}

/// Ревью одного файла
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReview {
  pub path: String,
  pub issues: Vec<Issue>,
}

#[derive(Deserialize, Debug)]
struct Response {
  choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
  message: Message,
}

#[derive(Deserialize, Debug)]
struct Message {
  content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmReview {
  pub issues: Vec<Issue>,
  pub errors: Vec<ReviewError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmReviewResponse {
  pub issues: Vec<Issue>,
}

pub async fn review_files(config: &LlmConfig, diffs: &[FileDiff]) -> Result<LlmReview> {
  let max_concurrent = 5;

  let reviews: Vec<(Vec<Issue>, Option<ReviewError>)> = stream::iter(diffs)
    .map(|file| review_single_file(config, file))
    .buffer_unordered(max_concurrent)
    .collect()
    .await;

  Ok(merge_reviews(reviews))
}

async fn review_single_file(
  config: &LlmConfig,
  file: &FileDiff,
) -> (Vec<Issue>, Option<ReviewError>) {
  let client = reqwest::Client::new();
  let prompt = build_prompt_single(file);

  let body = serde_json::json!({
      "model": config.model,
      "messages": [
          {
              "role": "system",
              "content": format!(r#"
                You are a senior software engineer performing automated code review.

                Your task is to analyze code changes and detect problems.

                First internally analyze the code step-by-step.
                Then return the final result as JSON. Do not skip any steps.

                When unsure, report a warning.

                You MUST detect and report:

                - syntax errors
                - type errors
                - logical bugs
                - unsafe code
                - incorrect async usage
                - incorrect error handling
                - potential runtime errors
                - dead code
                - performance issues
                - security issues
                - style violations

                If the code contains no issues, return an empty issues array.

                Rules:
                - Be strict.
                - Do not ignore potential problems.
                - Prefer reporting issues rather than skipping them.
                - Analyze carefully.

                You MUST return ONLY valid JSON.

                Output format:

                {{
                  "issues":[
                    {{
                      "file":"path/to/file",
                      "line":123,
                      "message":"short description of the problem",
                      "suggestion":"how to fix it",
                      "severity":"suggestion|warning|error",
                      "issue_type":"syntax|type|logic|style|performance|security"
                    }}
                  ]
                }}

                Do NOT include explanations outside JSON.
                Do NOT include markdown.
                Suggestions should be answered in Russian.
                Messages should be answered in Russian.
                Return JSON only.
              "#),
          },
          {
              "role": "user",
              "content": prompt
          }
      ],
      "temperature": 0,
      "response_format": {
        "type": "json_object"
      },
  });

  println!("Create request for {} file", file.path);
  let resp = client
    .post(&config.api_url)
    .bearer_auth(&config.api_key)
    .json(&body)
    .send()
    .await;

  let resp = match resp {
    Ok(r) => r,
    Err(e) => {
      return (
        vec![],
        Some(ReviewError {
          file: file.path.clone(),
          reason: format!("API request failed: {}", e),
        }),
      );
    }
  };

  println!("After request for {} file", file.path);
  let text = match resp.text().await {
    Ok(t) => t,
    Err(e) => {
      return (
        vec![],
        Some(ReviewError {
          file: file.path.clone(),
          reason: format!("Failed reading response: {}", e),
        }),
      );
    }
  };

  println!("Got repsonse for {} file", file.path);
  let json = match extract_json(&text) {
    Some(j) => j,
    None => {
      return (
        vec![],
        Some(ReviewError {
          file: file.path.clone(),
          reason: "LLM did not return JSON".into(),
        }),
      );
    }
  };

  let response = match serde_json::from_str::<Response>(&json) {
    Ok(r) => r,

    Err(e) => {
      return (
        vec![],
        Some(ReviewError {
          file: file.path.clone(),
          reason: format!("Invalid JSON: {}", e),
        }),
      );
    }
  };

  println!("Response for {} file: {:?}", file.path, response);
  match serde_json::from_str::<LlmReviewResponse>(&response.choices[0].message.content) {
    Ok(r) => (r.issues, None),

    Err(e) => (
      vec![],
      Some(ReviewError {
        file: file.path.clone(),
        reason: format!("Invalid JSON: {}", e),
      }),
    ),
  }
}

fn build_prompt_single(file: &FileDiff) -> String {
  format!(
    r#"
  Review the following file.

  FILE: {}

  CODE:

  {}
  
  "#,
    file.path, file.diff
  )
}

fn merge_reviews(results: Vec<(Vec<Issue>, Option<ReviewError>)>) -> LlmReview {
  let mut issues = Vec::new();
  let mut errors = Vec::new();

  for (i, e) in results {
    issues.extend(i);

    if let Some(err) = e {
      errors.push(err);
    }
  }

  LlmReview { issues, errors }
}

fn extract_json(text: &str) -> Option<String> {
  let start = text.find('{')?;
  let end = text.rfind('}')?;

  if end <= start {
    return None;
  }

  Some(text[start..=end].to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{config::LlmConfig, git::FileDiff};

  #[tokio::test]
  async fn review_single_file_real_llm_has_required_fields_when_issues_present() {
    let config = LlmConfig {
      api_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
      api_key: "sk-or-v1-"
        .to_string(),
      model: "openrouter/hunter-alpha".to_string(),
    };

    let file = FileDiff {
      path: "src/main.rs".to_string(),
      diff: "fn main() { println!(\"hello\"); let x = 1; Ok(x) }".to_string(),
    };

    let (issues, error) = review_single_file(&config, &file).await;

    println!("Issues: {:?}", issues);
    println!("Error: {:?}", error);
    // Если случилась ошибка сети/авторизации — явно падаем.
    assert!(
      error.is_none(),
      "expected no error from LLM, got: {:?}",
      error
    );

    // Модель может вернуть 0 issues — в этом случае тест не будет проверять поля.
    if issues.is_empty() {
      eprintln!("LLM returned no issues; JSON shape not validated in this run");
      return;
    }

    let issue = &issues[0];
    // Проверяем, что все ключевые поля из JSON присутствуют и распарсены.
    assert!(
      !issue.file.is_empty(),
      "issue.file should not be empty for real LLM response"
    );
    assert!(
      issue.line > 0,
      "issue.line should be > 0 when issue is reported"
    );
    assert!(
      !issue.message.is_empty(),
      "issue.message should not be empty"
    );
    assert!(
      !issue.suggestion.is_empty(),
      "issue.suggestion should not be empty"
    );
  }
}
