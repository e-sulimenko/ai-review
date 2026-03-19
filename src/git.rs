use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

const SUPPORTED_EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "jsx"];
const IGNORED_DIRS: &[&str] = &["node_modules", "dist", "build", "target", ".git"];

// Порог для маленького diff — если diff < N строк, отправляем весь файл
const DIFF_THRESHOLD: usize = 10;

#[derive(Debug, Clone)]
pub struct FileDiff {
  pub path: String,
  pub diff: String, // может быть diff или весь файл
}

/// Получаем все файлы для ревью
pub fn get_diff(branch: &str) -> Result<Vec<FileDiff>> {
  // Получаем diff по существующим файлам
  let output = Command::new("git")
    .args(&["diff", branch, "-U20"])
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

  // Применяем порог для существующих файлов
  for file in &mut files {
    if file.diff.lines().count() < DIFF_THRESHOLD {
      // Заглушка AST-блока: пока просто весь файл
      // Позже добавить Tree-sitter для извлечения функции/класса
      if let Ok(full_content) = fs::read_to_string(&file.path) {
        file.diff = full_content;
      }
    }
  }

  // Получаем новые (untracked) файлы
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
      // Для новых файлов берём весь файл
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
    } else if current_file.is_some() {
      current_diff.push_str(line);
      current_diff.push('\n');
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

/// Получаем имя текущей локальной ветки
pub fn current_branch() -> Result<String> {
  let output = Command::new("git")
    .args(&["rev-parse", "--abbrev-ref", "HEAD"])
    .output()
    .context("Failed to get current branch")?;

  if !output.status.success() {
    anyhow::bail!(
      "git rev-parse failed: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }

  let branch = String::from_utf8(output.stdout)?.trim().to_string();
  Ok(branch)
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
