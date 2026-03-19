use crate::review::ReviewSummary;
use anyhow::Result;
use serde_json;
use std::{
  env,
  fs,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

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
      for cl in &issue.code_lines {
        let marker = if cl.line == issue.line { ">>" } else { "  " };
        println!("{} {} | {}", marker, cl.line, cl.code);
      }
    }
  }
}

/// Вывод JSON (для --json)
pub fn print_json(summary: &ReviewSummary) -> Result<()> {
  let json = serde_json::to_string_pretty(summary)?;
  println!("{}", json);
  Ok(())
}

fn term_supports_osc8() -> bool {
  // Best-effort: OSC-8 hyperlinks are supported in iTerm2 and VSCode terminals.
  let term_program = env::var("TERM_PROGRAM").unwrap_or_default().to_lowercase();
  if term_program.contains("iterm") {
    return true;
  }
  if term_program.contains("vscode") {
    return true;
  }
  if env::var("VSCODE_IPC_HOOK_CLI").is_ok() {
    return true;
  }
  false
}

fn file_url_for_path(path: &Path) -> String {
  // Minimal escaping: spaces to %20.
  let abs = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
  let display = abs.to_string_lossy().replace(' ', "%20");
  format!("file://{}", display)
}

fn language_hint_for_path(path: &str) -> &'static str {
  let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
  match ext {
    "rs" => "rust",
    "toml" => "toml",
    "js" => "javascript",
    "ts" => "typescript",
    "py" => "python",
    "go" => "go",
    "java" => "java",
    "c" => "c",
    "cpp" | "cc" | "cxx" => "cpp",
    "sh" => "bash",
    "json" => "json",
    _ => "txt",
  }
}

fn sanitize_md_inline_text(s: &str) -> String {
  // Keep it simple: avoid HTML and prevent accidental markdown breaking by newlines.
  s.replace('\r', "").replace('\n', " ")
}

/// Generate markdown report and return absolute path to created file.
pub fn write_md_report(summary: &ReviewSummary) -> Result<PathBuf> {
  let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
  let file_name = format!("ai-review-{}.md", epoch);

  // Create relative to current working directory.
  let report_dir = PathBuf::from(".ai-review").join("reviews");
  fs::create_dir_all(&report_dir)?;

  let report_path = report_dir.join(file_name);
  let abs_report_path =
    fs::canonicalize(&report_path).unwrap_or_else(|_| report_path.clone());

  let mut md = String::new();
  md.push_str("# AI Code Review\n");
  md.push_str(&format!("Generated (unix epoch): {}\n", epoch));
  md.push_str(&format!("Files changed: {}\n", summary.files.len()));
  md.push_str(&format!("Lines changed: {}\n", summary.total_lines));
  md.push_str(&format!("Issues found: {}\n", summary.issues));
  md.push_str(&format!("Lines to fix: {}\n\n", summary.lines_to_fix));

  for file in &summary.files {
    if file.issues.is_empty() {
      continue;
    }

    md.push_str(&format!("## {}\n\n", file.path));
    let lang = language_hint_for_path(&file.path);

    for issue in &file.issues {
      md.push_str(&format!(
        "### Line {} [{}] ({})\n",
        issue.line,
        issue.severity.to_string(),
        issue.issue_type
      ));
      md.push_str(&format!(
        "**Message:** {}\n",
        sanitize_md_inline_text(&issue.message)
      ));
      md.push_str(&format!(
        "**Suggestion:** {}\n\n",
        sanitize_md_inline_text(&issue.suggestion)
      ));

      md.push_str(&format!("```{}\n", lang));
      for cl in &issue.code_lines {
        let marker = if cl.line == issue.line { ">>" } else { "  " };
        md.push_str(&format!("{} {} | {}\n", marker, cl.line, cl.code));
      }
      md.push_str("```\n\n");
    }
  }

  fs::write(&report_path, md)?;

  // Print path after file creation.
  let report_path_display = abs_report_path.display().to_string();
  if term_supports_osc8() {
    let url = file_url_for_path(&abs_report_path);
    let linked = format!(
      "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\",
      url, report_path_display
    );
    println!("MD report created: {}", linked);
  } else {
    println!("MD report created: {}", report_path_display);
  }

  Ok(abs_report_path)
}
