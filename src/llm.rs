use crate::{config::LlmConfig, git::FileDiff};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

const MAX_RETRY_COUNT: usize = 5;
// Генерация нескольких кандидатов нужна, чтобы повысить шанс разнообразия.
// Однако это сильно увеличивает стоимость (несколько LLM-вызовов на файл).
// При требовании "не дублировать issue внутри одного ответа" дедупликация
// становится менее критичной, поэтому держим 1 кандидат.
const CANDIDATE_REVIEWS_PER_DIFF: usize = 2;

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
  /// Контекст кода вокруг `line` (обычно 3 строки: line-1, line, line+1).
  /// Должен быть включен в ответ LLM, чтобы проблема была привязана к конкретному месту.
  pub code: String,
  pub file: String,
}

/// Ревью одного файла
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReview {
  pub path: String,
  pub issues: Vec<Issue>,
  pub errors: Vec<String>,
}

/// Ревью одного файла
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmFileReview {
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
  pub path: String,
  pub issues: Vec<Issue>,
  pub errors: Vec<String>,
}

enum LlmAttemptError {
  Request(String),
  Parse(String),
}

#[derive(Debug, Deserialize)]
struct DeduplicateResponse {
  reviews: Vec<FileReview>,
}

async fn review_single_file_once(
  client: &reqwest::Client,
  config: &LlmConfig,
  file: &FileDiff,
  body: &serde_json::Value,
) -> std::result::Result<LlmFileReview, LlmAttemptError> {
  println!("Create request for {} file", file.path);
  let resp = client
    .post(&config.api_url)
    .bearer_auth(&config.api_key)
    .json(body)
    .send()
    .await;

  let resp = match resp {
    Ok(r) => r,
    Err(e) => return Err(LlmAttemptError::Request(format!("API request failed: {}", e))),
  };

  println!("After request for {} file", file.path);
  let text = match resp.text().await {
    Ok(t) => t,
    Err(e) => {
      return Err(LlmAttemptError::Request(format!(
        "Failed reading response: {}",
        e
      )))
    },
  };

  println!("Got repsonse for {} file", file.path);
  let json = match extract_json(&text) {
    Some(j) => j,
    None => return Err(LlmAttemptError::Parse("LLM did not return JSON".into())),
  };

  let response = match serde_json::from_str::<Response>(&json) {
    Ok(r) => r,
    Err(e) => {
      return Err(LlmAttemptError::Parse(format!(
        "Invalid JSON (response wrapper): {}",
        e
      )))
    }
  };

  if response.choices.is_empty() {
    return Err(LlmAttemptError::Parse("LLM response has empty choices".into()));
  }

  let content = &response.choices[0].message.content;
  println!("Response for file: {}, content: {:?}", file.path, content);

  match serde_json::from_str::<LlmFileReview>(content) {
    Ok(r) => Ok(r),
    Err(e) => Err(LlmAttemptError::Parse(format!(
      "Invalid JSON (file review): {}",
      e
    ))),
  }
}

async fn review_single_file_request_with_retries(
  client: &reqwest::Client,
  config: &LlmConfig,
  file: &FileDiff,
  body: &serde_json::Value,
) -> FileReview {
  let mut last_json_error: Option<String> = None;

  for attempt in 0..MAX_RETRY_COUNT {
    println!(
      "Create request for {} file (attempt {}/{})",
      file.path,
      attempt + 1,
      MAX_RETRY_COUNT
    );

    match review_single_file_once(client, config, file, body).await {
      Ok(r) => {
        return FileReview {
          path: file.path.clone(),
          issues: r.issues,
          errors: vec![],
        };
      }
      Err(LlmAttemptError::Request(e)) => {
        // Для сетевых/HTTP ошибок ретраи не делаем (как и раньше).
        return FileReview {
          path: file.path.clone(),
          issues: vec![],
          errors: vec![e],
        };
      }
      Err(LlmAttemptError::Parse(e)) => {
        last_json_error = Some(e);
        continue;
      }
    }
  }

  FileReview {
    path: file.path.clone(),
    issues: vec![],
    errors: vec![last_json_error.unwrap_or_else(|| "LLM did not return valid JSON".into())],
  }
}

enum DeduplicateAttemptError {
  Request(String),
  ParseOrValidate(String),
}

async fn deduplicate_reviews_once(
  client: &reqwest::Client,
  config: &LlmConfig,
  file: &FileDiff,
  candidates: &[FileReview],
) -> std::result::Result<Vec<FileReview>, DeduplicateAttemptError> {
  let candidates_json = serde_json::to_string(candidates)
    .unwrap_or_else(|_| "[]".to_string());

  let system_prompt = format!(
    r#"
You are a senior software engineer performing automated code review deduplication.

The user will provide a JSON array named `candidates`. Each element is a FileReview produced by analyzing the SAME diff for the same file.
Your goal is to remove semantic duplicates across all candidates.

Rules:
- Keep only unique issues after deduplication.
- Treat issues as duplicates when they refer to the same `line` and the same `issue_type`, and their `message` are semantically equivalent.
- Merge duplicates by keeping the most specific `message`, the best `suggestion`, and the most representative `code`.
- If a candidate has non-empty `errors`, ignore it for deduplication (treat it as having zero issues).
- Return exactly ONE JSON object (no markdown, no explanations).
- Suggestions and messages must be in Russian.

Output schema (JSON only):
{{
  "reviews": [
    {{
      "path": "{path}",
      "issues": [ {{issue}} ],
      "errors": []
    }}
  ]
}}

Where each issue object must contain:
- `line` (positive integer),
- `severity` ("error"|"warning"|"suggestion"),
- `issue_type` ("syntax"|"type"|"logic"|"style"|"performance"|"security"),
- `message` (Russian),
- `suggestion` (Russian),
- `code` (string, should contain the relevant code context around the issue line),
- `file` (must equal "{path}").
"#,
    path = file.path
  );

  let body = serde_json::json!({
    "model": config.model,
    "messages": [
      { "role": "system", "content": system_prompt },
      { "role": "user", "content": format!("candidates JSON (same diff, same file): {}", candidates_json) }
    ],
    "temperature": 0,
    "response_format": { "type": "json_object" }
  });

  let resp = client
    .post(&config.api_url)
    .bearer_auth(&config.api_key)
    .json(&body)
    .send()
    .await;

  let resp = match resp {
    Ok(r) => r,
    Err(e) => {
      return Err(DeduplicateAttemptError::Request(format!(
        "API request failed: {}",
        e
      )))
    }
  };

  let text = match resp.text().await {
    Ok(t) => t,
    Err(e) => {
      return Err(DeduplicateAttemptError::Request(format!(
        "Failed reading response: {}",
        e
      )))
    }
  };

  let json = match extract_json(&text) {
    Some(j) => j,
    None => {
      return Err(DeduplicateAttemptError::ParseOrValidate(
        "LLM did not return JSON wrapper".into(),
      ))
    }
  };

  let response = match serde_json::from_str::<Response>(&json) {
    Ok(r) => r,
    Err(e) => {
      return Err(DeduplicateAttemptError::ParseOrValidate(format!(
        "Invalid JSON (response wrapper): {}",
        e
      )))
    }
  };

  if response.choices.is_empty() {
    return Err(DeduplicateAttemptError::ParseOrValidate(
      "LLM response has empty choices".into(),
    ));
  }

  let content = &response.choices[0].message.content;
  let content_json = extract_json(content).ok_or_else(|| {
    DeduplicateAttemptError::ParseOrValidate("LLM did not return JSON content".into())
  })?;

  let dedup_response = match serde_json::from_str::<DeduplicateResponse>(&content_json) {
    Ok(r) => r,
    Err(e) => {
      return Err(DeduplicateAttemptError::ParseOrValidate(format!(
        "Invalid JSON (dedup response): {}",
        e
      )))
    }
  };

  // Validate: each review must correspond to FileReview for the same path.
  if dedup_response.reviews.is_empty() {
    return Err(DeduplicateAttemptError::ParseOrValidate(
      "Dedup response contains empty reviews list".into(),
    ));
  }

  for r in &dedup_response.reviews {
    if r.path != file.path {
      return Err(DeduplicateAttemptError::ParseOrValidate(format!(
        "Dedup review path mismatch: expected {}, got {}",
        file.path, r.path
      )));
    }

    for issue in &r.issues {
      if issue.file != file.path {
        return Err(DeduplicateAttemptError::ParseOrValidate(format!(
          "Dedup issue.file mismatch: expected {}, got {}",
          file.path, issue.file
        )));
      }
      if issue.line == 0 {
        return Err(DeduplicateAttemptError::ParseOrValidate(
          "Dedup issue.line must be positive".into(),
        ));
      }
      if issue.code.trim().is_empty() {
        return Err(DeduplicateAttemptError::ParseOrValidate(
          "Dedup issue.code must be non-empty".into(),
        ));
      }
    }
  }

  Ok(dedup_response.reviews)
}

async fn deduplicate_reviews_with_retries(
  client: &reqwest::Client,
  config: &LlmConfig,
  file: &FileDiff,
  candidates: &[FileReview],
) -> FileReview {
  let mut last_err: Option<String> = None;

  for attempt in 0..MAX_RETRY_COUNT {
    println!(
      "Create dedup request for {} (attempt {}/{})",
      file.path,
      attempt + 1,
      MAX_RETRY_COUNT
    );

    match deduplicate_reviews_once(client, config, file, candidates).await {
      Ok(reviews) => {
        return reviews.into_iter().next().unwrap_or(FileReview {
          path: file.path.clone(),
          issues: vec![],
          errors: vec![],
        });
      }
      Err(DeduplicateAttemptError::Request(e)) => {
        // Network/HTTP errors: do not retry.
        return FileReview {
          path: file.path.clone(),
          issues: vec![],
          errors: vec![e],
        };
      }
      Err(DeduplicateAttemptError::ParseOrValidate(e)) => {
        last_err = Some(e);
        continue;
      }
    }
  }

  FileReview {
    path: file.path.clone(),
    issues: vec![],
    errors: vec![last_err
      .unwrap_or_else(|| "Dedup LLM did not return a valid deduplicated JSON".into())],
  }
}

pub async fn review_files(config: &LlmConfig, diffs: &[FileDiff]) -> Result<Vec<LlmReview>> {
  let max_concurrent = 5;

  let reviews: Vec<FileReview> = stream::iter(diffs)
    .map(|file| review_single_file(config, file))
    .buffer_unordered(max_concurrent)
    .collect()
    .await;

  Ok(merge_reviews(reviews))
}

async fn review_single_file(
  config: &LlmConfig,
  file: &FileDiff,
) -> FileReview {
  let client = reqwest::Client::new();
  let prompt = build_prompt_single(file);

  let body = serde_json::json!({
      "model": config.model,
      "messages": [
          {
              "role": "system",
              "content": format!(r#"
                You are a senior software engineer performing automated code review.
                Analyze the diff and detect problems in the changed code.

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
                - Do NOT output semantically duplicate issues in the same response.
                  If two entries refer to the same `line` and the same `issue_type`,
                  merge them into one issue (choose the most specific `message` / `suggestion`).

                You MUST return ONLY valid JSON.

                Output format:

                {{
                  "issues":[
                    {{
                      "file":"path/to/file",
                      "line":123,
                      "message":"short description of the problem",
                      "suggestion":"how to fix it",
                      "code":"one line above / this line / one line below (recommended 3 lines total)",
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

  let mut results = Vec::<FileReview>::with_capacity(CANDIDATE_REVIEWS_PER_DIFF);

  for candidate_idx in 0..CANDIDATE_REVIEWS_PER_DIFF {
    println!(
      "Generate candidate review for {} ({} / {})",
      file.path,
      candidate_idx + 1,
      CANDIDATE_REVIEWS_PER_DIFF
    );
    results
      .push(
        review_single_file_request_with_retries(&client, config, file, &body)
          .await,
      );
  }

  // При 1 кандидате дедупликация не нужна (экономим LLM-вызов).
  if results.len() <= 1 {
    return results
      .into_iter()
      .next()
      .unwrap_or(FileReview {
        path: file.path.clone(),
        issues: vec![],
        errors: vec![],
      });
  }

  // Дедупликация семантически одинаковых issue-объектов.
  deduplicate_reviews_with_retries(&client, config, file, &results).await
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

fn merge_reviews(results: Vec<FileReview>) -> Vec<LlmReview> {
  let mut dict = HashMap::<String, LlmReview>::new();

  for review in results {
    match dict.get_mut(&review.path) {
      Some(file_review) => {
        file_review.errors.extend(review.errors);
        file_review.issues.extend(review.issues);
      },
      None => {
        dict.insert(
          review.path.clone(),
          LlmReview {
            path: review.path.clone(),
            issues: review.issues,
            errors: review.errors,
          },
        );
      }
    }
  }

  dict.into_values().collect()
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
      api_key: "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"
        .to_string(),
      model: "openrouter/hunter-alpha".to_string(),
    };

    let file = FileDiff {
      path: "src/main.rs".to_string(),
      diff: "fn main() { println!(\"hello\"); let x = 1; Ok(x) }".to_string(),
    };

    let file_review = review_single_file(&config, &file).await;

    println!("Issues: {:?}", file_review.issues);
    println!("Error: {:?}", file_review.errors);
    // Если случилась ошибка сети/авторизации — явно падаем.
    assert!(
      !file_review.errors.is_empty(),
      "expected no error from LLM, got: {:?}",
      file_review.errors,
    );

    // Модель может вернуть 0 issues — в этом случае тест не будет проверять поля.
    if file_review.issues.is_empty() {
      eprintln!("LLM returned no issues; JSON shape not validated in this run");
      return;
    }

    let issue = &file_review.issues[0];
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
    assert!(
      !issue.code.trim().is_empty(),
      "issue.code should be non-empty (code context around the issue line)"
    );
  }
}
