use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::fs;

/// Поддерживаемые расширения файлов
const SUPPORTED_EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "jsx"];

/// Игнорируемые директории
const IGNORED_DIRS: &[&str] = &["node_modules", "dist", "build", "target", ".git"];

/// Структура для хранения diff по файлу
#[derive(Debug, Clone)]
pub struct FileDiff {
  pub path: String,
  pub diff: String,
}

/// Получение diff относительно origin/main
pub fn get_diff() -> Result<Vec<FileDiff>> {
  // Обновляем origin/main
  // Command::new("git")
  //   .args(&["fetch", "origin", "main"])
  //   .status()
  //   .context("Failed to fetch origin/main")?;

  // Получаем diff
  let output = Command::new("git")
    .args(&["diff", "master", "-U20"])
    .output()
    .context("Failed to run git diff")?;

  if !output.status.success() {
    anyhow::bail!(
      "git diff failed: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }

  let diff_text = String::from_utf8(output.stdout).context("Diff is not valid UTF-8")?;

  let mut files = parse_diff(&diff_text);

  let new_files_output = Command::new("git")
    .args(&["ls-files", "--others", "--exclude-standard"])
    .output()
    .context("Failed to get untracked files")?;
  if !new_files_output.status.success() {
    anyhow::bail!(
      "git ls-files failed: {}",
      String::from_utf8_lossy(&new_files_output.stderr)
    );
  }
  
  let new_files_text = String::from_utf8(new_files_output.stdout)?;
  for line in new_files_text.lines() {
    let path = line.trim();
    if is_supported(path) && !is_ignored(path) {
      // читаем содержимое файла как diff
      let content = fs::read_to_string(path).unwrap_or_default();
      files.push(FileDiff {
        path: path.to_string(),
        diff: content,
      });
    }
  }

  Ok(files)
}

/// Парсинг diff и фильтрация файлов
fn parse_diff(diff_text: &str) -> Vec<FileDiff> {
  let mut files = Vec::new();
  let mut current_file: Option<String> = None;
  let mut current_diff = String::new();

  for line in diff_text.lines() {
    if let Some(stripped) = line.strip_prefix("+++ b/") {
      // Сохраняем предыдущий файл
      if let Some(path) = current_file.take() {
        if is_supported(&path) && !is_ignored(&path) {
          files.push(FileDiff {
            path,
            diff: current_diff.clone(),
          });
        }
        current_diff.clear();
      }
      current_file = Some(stripped.to_string());
    } else {
      if current_file.is_some() {
        current_diff.push_str(line);
        current_diff.push('\n');
      }
    }
  }

  // Добавляем последний файл
  if let Some(path) = current_file {
    if is_supported(&path) && !is_ignored(&path) {
      files.push(FileDiff {
        path,
        diff: current_diff,
      });
    }
  }

  files
}

/// Проверка расширения файла
fn is_supported(path: &str) -> bool {
  SUPPORTED_EXTENSIONS.iter().any(|ext| path.ends_with(ext))
}

/// Проверка игнорируемых директорий
fn is_ignored(path: &str) -> bool {
  IGNORED_DIRS
    .iter()
    .any(|dir| Path::new(path).components().any(|c| c.as_os_str() == *dir))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_supported() {
    assert!(is_supported("src/main.rs"));
    assert!(is_supported("lib/index.ts"));
    assert!(!is_supported("README.md"));
  }

  #[test]
  fn test_ignored() {
    assert!(is_ignored("node_modules/foo.js"));
    assert!(is_ignored("build/main.rs"));
    assert!(!is_ignored("src/main.rs"));
  }

  #[test]
  fn test_parse_diff() {
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 123..456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
+use anyhow;
 fn main() {}
";
    let files = parse_diff(diff);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "src/main.rs");
    assert!(files[0].diff.contains("use anyhow;"));
  }
}
