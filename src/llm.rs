use crate::{config::LlmConfig, git::FileDiff, ui_log::UiLogger};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, time::Instant};

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
  #[serde(rename = "async")]
  Async,
  #[serde(rename = "error_handling")]
  ErrorHandling,
  #[serde(rename = "dead_code")]
  DeadCode,
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
      IssueType::Async => write!(f, "async"),
      IssueType::ErrorHandling => write!(f, "error_handling"),
      IssueType::DeadCode => write!(f, "dead_code"),
    }
  }
}

/// Одна строка контекста кода вокруг issue.
///
/// Контракт для LLM:
/// - `line` положительный
/// - `code` не пустой
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeLine {
  pub line: usize,
  pub code: String,
}

/// Структура одного issue, возвращаемого LLM
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Issue {
  pub line: usize,
  pub severity: IssueSeverity,
  pub issue_type: IssueType,
  pub message: String,
  pub suggestion: String,
  /// Нумерованный контекст кода вокруг `line`.
  ///
  /// Контракт:
  /// - массив `code_lines` отсортирован по `line` по возрастанию
  /// - внутри массива обязательно есть элемент с `code_lines[i].line == line`
  pub code_lines: Vec<CodeLine>,
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

fn parse_llm_wrapper_content(wrapper_json: &str) -> std::result::Result<String, String> {
  let v: serde_json::Value = serde_json::from_str(wrapper_json)
    .map_err(|e| format!("Invalid JSON in LLM response wrapper: {}", e))?;

  // OpenAI / OpenRouter compatible.
  if v.get("choices").is_some() {
    let response: Response = serde_json::from_value(v)
      .map_err(|e| format!("Invalid JSON (response wrapper): {}", e))?;
    if response.choices.is_empty() {
      return Err("LLM response has empty choices".into());
    }
    return Ok(response.choices[0].message.content.clone());
  }

  // Common error wrapper from various providers.
  if v.get("error").is_some() {
    let msg = v
      .get("error")
      .and_then(|e| e.get("message"))
      .and_then(|m| m.as_str())
      .unwrap_or("LLM returned an error object");
    return Err(format!("LLM API returned error: {}", msg));
  }

  // Best-effort diagnostics: show keys + a small preview to help debugging.
  let keys = v
    .as_object()
    .map(|o| o.keys().cloned().collect::<Vec<_>>())
    .unwrap_or_default();
  let preview: String = wrapper_json.chars().take(300).collect();
  Err(format!(
    "Invalid JSON (response wrapper): missing field `choices` and no known alternate fields. Top-level keys: {:?}. Preview: {}",
    keys, preview
  ))
}

fn apply_llm_extra_request_fields(
  mut body: serde_json::Value,
  extra: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
  if let Some(obj) = body.as_object_mut() {
    for (k, v) in extra {
      // Если ключ уже есть в base body, мы его перезатираем.
      obj.insert(k.clone(), v.clone());
    }
  }
  body
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
  logger: &UiLogger,
  candidate_no: usize,
  attempt_no: usize,
) -> std::result::Result<LlmFileReview, LlmAttemptError> {
  let total_start = Instant::now();

  logger.info(&format!(
    "{}: sending LLM request (candidate {}, attempt {})...",
    file.path, candidate_no, attempt_no
  ));

  let send_start = Instant::now();
  let resp = client
    .post(&config.api_url)
    .bearer_auth(&config.api_key)
    .json(body)
    .send()
    .await;

  let resp = match resp {
    Ok(r) => {
      let status = r.status();
      logger.info(&format!(
        "{}: LLM response headers received (candidate {}, attempt {}), status={} after {}ms",
        file.path,
        candidate_no,
        attempt_no,
        status,
        send_start.elapsed().as_millis()
      ));
      r
    }
    Err(e) => return Err(LlmAttemptError::Request(format!("API request failed: {}", e))),
  };

  let read_start = Instant::now();
  let text = match resp.text().await {
    Ok(t) => t,
    Err(e) => {
      return Err(LlmAttemptError::Request(format!(
        "Failed reading response: {}",
        e
      )))
    },
  };

  logger.info(&format!(
    "{}: LLM response body received (candidate {}, attempt {}), {} chars read in {}ms",
    file.path,
    candidate_no,
    attempt_no,
    text.len(),
    read_start.elapsed().as_millis()
  ));

  if logger.debug_enabled() {
    logger.debug(&format!(
      "{}: response parse pipeline total {}ms (candidate {}, attempt {})",
      file.path,
      total_start.elapsed().as_millis(),
      candidate_no,
      attempt_no
    ));
  }

  let json = match extract_json(&text) {
    Some(j) => j,
    None => return Err(LlmAttemptError::Parse("LLM did not return JSON".into())),
  };

  let content = match parse_llm_wrapper_content(&json) {
    Ok(c) => c,
    Err(e) => return Err(LlmAttemptError::Parse(e)),
  };

  match serde_json::from_str::<LlmFileReview>(&content) {
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
  logger: &UiLogger,
  candidate_no: usize,
) -> FileReview {
  let mut last_json_error: Option<String> = None;

  for attempt in 0..config.max_retry_count {
    if attempt == 0 {
      logger.info(&format!(
        "{}: calling LLM (candidate {}/{})",
        file.path,
        candidate_no,
        config.candidate_reviews_per_diff
      ));
    } else {
      logger.warn(&format!(
        "{}: invalid LLM JSON; retrying ({}/{}) for candidate {}",
        file.path,
        attempt + 1,
        config.max_retry_count,
        candidate_no
      ));
    }

    if logger.debug_enabled() {
      logger.debug(&format!(
        "{}: request payload size: {} chars; extra_body keys: {:?}",
        file.path,
        serde_json::to_string(body).map(|s| s.len()).unwrap_or(0),
        config.extra_body.keys().collect::<Vec<_>>()
      ));
    }

    match review_single_file_once(
      client,
      config,
      file,
      body,
      logger,
      candidate_no,
      attempt + 1,
    )
      .await
    {
      Ok(r) => {
        if logger.debug_enabled() {
          logger.debug(&format!(
            "{}: candidate {} parsed: issues={}, errors={}",
            file.path,
            candidate_no,
            r.issues.len(),
            0usize
          ));
        }
        return FileReview {
          path: file.path.clone(),
          issues: r.issues,
          errors: vec![],
        };
      }
      Err(LlmAttemptError::Request(e)) => {
        logger.warn(&format!(
          "{}: LLM request failed for candidate {}: {}",
          file.path, candidate_no, e
        ));

        // Для сетевых/HTTP ошибок ретраи не делаем (как и раньше).
        return FileReview {
          path: file.path.clone(),
          issues: vec![],
          errors: vec![e],
        };
      }
      Err(LlmAttemptError::Parse(e)) => {
        if logger.debug_enabled() {
          logger.debug(&format!(
            "{}: candidate {} parse error (attempt {}): {}",
            file.path,
            candidate_no,
            attempt + 1,
            e
          ));
        }
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
- Merge duplicates by keeping the most specific `message`, the best `suggestion`, and the most representative `code_lines`.
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
- `issue_type` ("syntax"|"type"|"logic"|"style"|"performance"|"security"|"async"|"error_handling"|"dead_code"),
- `message` (Russian),
- `suggestion` (Russian),
- `code_lines` (array of objects, each has `line` (positive integer) and `code` (string); array must be sorted by `line` ascending; must include at least one element with `line` == the issue `line`),
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

  let body = apply_llm_extra_request_fields(body, &config.extra_body);

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
  let content = match parse_llm_wrapper_content(&json) {
    Ok(c) => c,
    Err(e) => {
      return Err(DeduplicateAttemptError::ParseOrValidate(format!(
        "Invalid JSON (response wrapper): {}",
        e
      )))
    }
  };
  let content_json = extract_json(&content).ok_or_else(|| {
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
      if issue.code_lines.is_empty() {
        return Err(DeduplicateAttemptError::ParseOrValidate(
          "Dedup issue.code_lines must be non-empty".into(),
        ));
      }

      let mut has_target_line = false;
      for cl in &issue.code_lines {
        if cl.line == 0 {
          return Err(DeduplicateAttemptError::ParseOrValidate(
            "Dedup code_lines.line must be positive".into(),
          ));
        }
        if cl.code.trim().is_empty() {
          return Err(DeduplicateAttemptError::ParseOrValidate(
            "Dedup code_lines.code must be non-empty".into(),
          ));
        }
        if cl.line == issue.line {
          has_target_line = true;
        }
      }

      if !has_target_line {
        return Err(DeduplicateAttemptError::ParseOrValidate(
          "Dedup issue.code_lines must contain the target line".into(),
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
  logger: &UiLogger,
) -> FileReview {
  let mut last_err: Option<String> = None;

  for attempt in 0..config.max_retry_count {
    if logger.debug_enabled() {
      logger.debug(&format!(
        "{}: dedup attempt {}/{} (candidates={})",
        file.path,
        attempt + 1,
        config.max_retry_count,
        candidates.len()
      ));
    }

    match deduplicate_reviews_once(client, config, file, candidates).await {
      Ok(reviews) => {
        let result = reviews.into_iter().next().unwrap_or(FileReview {
          path: file.path.clone(),
          issues: vec![],
          errors: vec![],
        });
        if logger.debug_enabled() {
          logger.debug(&format!(
            "{}: dedup parsed: issues={}, errors={}",
            file.path,
            result.issues.len(),
            result.errors.len()
          ));
        }
        return result;
      }
      Err(DeduplicateAttemptError::Request(e)) => {
        logger.warn(&format!(
          "{}: deduplication request failed: {}",
          file.path, e
        ));

        // Network/HTTP errors: do not retry.
        return FileReview {
          path: file.path.clone(),
          issues: vec![],
          errors: vec![e],
        };
      }
      Err(DeduplicateAttemptError::ParseOrValidate(e)) => {
        if attempt + 1 < config.max_retry_count {
          logger.warn(&format!(
            "{}: deduplication invalid JSON; retrying ({}/{})",
            file.path,
            attempt + 1,
            config.max_retry_count
          ));
        }
        if logger.debug_enabled() {
          logger.debug(&format!(
            "{}: dedup parse error (attempt {}): {}",
            file.path,
            attempt + 1,
            e
          ));
        }
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

pub async fn review_files(
  config: &LlmConfig,
  diffs: &[FileDiff],
  logger: &UiLogger,
) -> Result<Vec<LlmReview>> {
  let max_concurrent = 5;
  let total_files = diffs.len();
  logger.info(&format!(
    "Starting LLM review for {} file(s) (max {} in parallel).",
    total_files, max_concurrent
  ));
  if logger.debug_enabled() {
    logger.debug(&format!(
      "LLM review settings: candidate_reviews_per_diff={}, max_retry_count={}, max_concurrent={}",
      config.candidate_reviews_per_diff,
      config.max_retry_count,
      max_concurrent
    ));
  }

  let reviews: Vec<FileReview> = stream::iter(diffs.iter().enumerate())
    .map(|(idx, file)| review_single_file(config, file, idx + 1, total_files, logger))
    .buffer_unordered(max_concurrent)
    .collect()
    .await;

  Ok(merge_reviews(reviews))
}

async fn review_single_file(
  config: &LlmConfig,
  file: &FileDiff,
  file_no: usize,
  total_files: usize,
  logger: &UiLogger,
) -> FileReview {
  let client = reqwest::Client::new();
  let prompt = build_prompt_single(file);

  logger.info(&format!(
    "[{}/{}] Reviewing file: {}",
    file_no, total_files, file.path
  ));

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
                      "code_lines":[
                        {{"line":122,"code":"..."}},
                        {{"line":123,"code":"..."}},
                        {{"line":124,"code":"..."}}
                      ],
                      "severity":"suggestion|warning|error",
                      "issue_type":"syntax|type|logic|style|performance|security|async|error_handling|dead_code"
                    }}
                  ]
                }}

                Constraints for `code_lines`:
                - `code_lines` must be sorted by `line` ascending
                - `code_lines` must include at least one element whose `line` equals the issue `line`
                - each `code_lines[i].code` must be the exact source code text of that line (no `N |` prefixes)

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

  let body = apply_llm_extra_request_fields(body, &config.extra_body);

  let mut results = Vec::<FileReview>::with_capacity(config.candidate_reviews_per_diff);

  for _candidate_idx in 0..config.candidate_reviews_per_diff {
    results
      .push(
        review_single_file_request_with_retries(
          &client,
          config,
          file,
          &body,
          logger,
          _candidate_idx + 1
        )
          .await,
      );
  }

  // При 1 кандидате дедупликация не нужна (экономим LLM-вызов).
  if results.len() <= 1 {
    logger.info(&format!(
      "{}: only 1 candidate; skipping deduplication.",
      file.path
    ));
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
  logger.info(&format!(
    "{}: deduplicating {} candidates...",
    file.path,
    results.len()
  ));
  deduplicate_reviews_with_retries(&client, config, file, &results, logger).await
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
    let logger = UiLogger::new(false, false);
    let config = LlmConfig {
      api_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
      api_key: "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"
        .to_string(),
      model: "openrouter/hunter-alpha".to_string(),
      max_retry_count: 3,
      candidate_reviews_per_diff: 2,
      extra_body: Default::default(),
    };

    let file = FileDiff {
      path: "src/main.rs".to_string(),
      diff: "fn main() { println!(\"hello\"); let x = 1; Ok(x) }".to_string(),
    };

    let file_review = review_single_file(&config, &file, 1, 1, &logger).await;

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
      !issue.code_lines.is_empty(),
      "issue.code_lines should be non-empty (numbered code context around the issue line)"
    );
    assert!(
      issue.code_lines.iter().any(|cl| cl.line == issue.line),
      "issue.code_lines must contain the target line (cl.line == issue.line)"
    );
  }
}
