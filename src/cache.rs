use crate::llm::LlmReview;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Каталог по умолчанию относительно cwd: `.ai-review/cache`.
pub fn default_cache_dir() -> &'static Path {
  Path::new(".ai-review/cache")
}

/// Удаляет каталог кеша целиком, если он существует.
pub fn clear_cache_dir(cache_root: &Path) -> anyhow::Result<()> {
  if cache_root.exists() {
    fs::remove_dir_all(cache_root)
      .map_err(|e| anyhow!("remove review cache dir {}: {e}", cache_root.display()))?;
  }
  Ok(())
}

/// Канонизация пути ревью для стабильного имени файла кеша и поля `path` в записи.
///
/// — обрезка пробелов по краям  
/// — `\` → `/`  
/// — убираем ведущие `./`  
/// — схлопываем `.`, пустые сегменты; `..` поднимает уровень (как в POSIX)
pub fn canonical_review_path(raw: &str) -> String {
  let raw = raw.trim().replace('\\', "/");
  let raw = raw.trim_start_matches("./");
  let mut parts: Vec<&str> = Vec::new();
  for part in raw.split('/') {
    if part.is_empty() || part == "." {
      continue;
    }
    if part == ".." {
      parts.pop();
    } else {
      parts.push(part);
    }
  }
  if parts.is_empty() {
    return ".".to_string();
  }
  parts.join("/")
}

/// SHA-256(UTF-8 канонического пути) в hex — имя файла кеша без расширения.
pub fn path_hash_hex(canonical_path: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(canonical_path.as_bytes());
  format!("{:x}", hasher.finalize())
}

/// SHA-256(UTF-8 содержимого диффа/полезной нагрузки) в hex — поле `key` в JSON.
pub fn diff_key_hex(diff_payload: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(diff_payload.as_bytes());
  format!("{:x}", hasher.finalize())
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
  /// Канонический путь; при чтении должен совпадать с текущим файлом (защита от коллизий хеша).
  path: String,
  /// Хеш полезной нагрузки (той же, что уходит в LLM).
  key: String,
  review: LlmReview,
}

/// Результат чтения кеша: либо попадание, либо промах; `diagnostic` — предупреждение для лога.
pub struct CacheReadOutcome {
  pub review: Option<LlmReview>,
  pub diagnostic: Option<String>,
}

fn cache_file_path(cache_root: &Path, canonical_path: &str) -> std::path::PathBuf {
  cache_root.join(format!("{}.json", path_hash_hex(canonical_path)))
}

pub fn read_cached_review(
  cache_root: &Path,
  raw_path: &str,
  diff_payload: &str,
) -> CacheReadOutcome {
  let canon = canonical_review_path(raw_path);
  let file = cache_file_path(cache_root, &canon);
  if !file.is_file() {
    return CacheReadOutcome {
      review: None,
      diagnostic: None,
    };
  }

  let data = match fs::read_to_string(&file) {
    Ok(d) => d,
    Err(e) => {
      return CacheReadOutcome {
        review: None,
        diagnostic: Some(format!(
          "cache read failed for {}: {}",
          file.display(),
          e
        )),
      };
    }
  };

  let entry: CacheEntry = match serde_json::from_str(&data) {
    Ok(e) => e,
    Err(e) => {
      return CacheReadOutcome {
        review: None,
        diagnostic: Some(format!(
          "cache JSON invalid for {}: {}",
          file.display(),
          e
        )),
      };
    }
  };

  if entry.path != canon {
    return CacheReadOutcome {
      review: None,
      diagnostic: Some(format!(
        "cache path hash collision or stale entry: file {} maps to path `{}` but cache \
         contains `{}`; refusing to use this cache entry",
        file.display(),
        canon,
        entry.path
      )),
    };
  }

  let current_key = diff_key_hex(diff_payload);
  if entry.key != current_key {
    return CacheReadOutcome {
      review: None,
      diagnostic: None,
    };
  }

  CacheReadOutcome {
    review: Some(entry.review),
    diagnostic: None,
  }
}

#[derive(Debug)]
pub enum CacheWriteError {
  PathHashCollision {
    cache_file: std::path::PathBuf,
    existing_canonical_path: String,
    our_canonical_path: String,
  },
  Other(anyhow::Error),
}

impl std::fmt::Display for CacheWriteError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CacheWriteError::PathHashCollision {
        cache_file,
        existing_canonical_path,
        our_canonical_path,
      } => write!(
        f,
        "cache file {} already holds path `{}` but we write `{}` (SHA-256 path collision)",
        cache_file.display(),
        existing_canonical_path,
        our_canonical_path
      ),
      CacheWriteError::Other(e) => write!(f, "{:#}", e),
    }
  }
}

impl std::error::Error for CacheWriteError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      CacheWriteError::Other(e) => Some(e.as_ref()),
      _ => None,
    }
  }
}

/// Записать кеш: одна запись на канонический путь; при несовпадении `path` в существующем файле —
/// коллизия хеша имён, запись не выполняется.
pub fn write_cached_review(
  cache_root: &Path,
  raw_path: &str,
  diff_payload: &str,
  review: &LlmReview,
) -> std::result::Result<(), CacheWriteError> {
  let canon = canonical_review_path(raw_path);
  let key = diff_key_hex(diff_payload);
  fs::create_dir_all(cache_root)
    .map_err(|e| {
      CacheWriteError::Other(anyhow!("create review cache directory: {e}"))
    })?;

  let file = cache_file_path(cache_root, &canon);

  if file.is_file() {
    let data = match fs::read_to_string(&file) {
      Ok(d) => d,
      Err(e) => {
        return Err(CacheWriteError::Other(anyhow!(
          "read existing cache before write: {e}"
        )));
      }
    };
    if let Ok(existing) = serde_json::from_str::<CacheEntry>(&data) {
      if existing.path != canon {
        return Err(CacheWriteError::PathHashCollision {
          cache_file: file,
          existing_canonical_path: existing.path,
          our_canonical_path: canon,
        });
      }
    }
  }

  let entry = CacheEntry {
    path: canon.clone(),
    key,
    review: LlmReview {
      path: canon,
      issues: review.issues.clone(),
      errors: review.errors.clone(),
    },
  };

  let json =
    serde_json::to_string(&entry).map_err(|e| CacheWriteError::Other(e.into()))?;
  fs::write(&file, json).map_err(|e| {
    CacheWriteError::Other(anyhow!(
      "write review cache file {}: {e}",
      file.display()
    ))
  })?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn canonical_path_normalizes_slashes_and_dots() {
    assert_eq!(
      canonical_review_path(r".\foo\bar\.\baz"),
      "foo/bar/baz"
    );
    assert_eq!(
      canonical_review_path("./src/../src/x.rs"),
      "src/x.rs"
    );
    assert_eq!(canonical_review_path("  ./a//b  "), "a/b");
  }

  #[test]
  fn path_hash_stable_for_equivalent_paths() {
    let a = canonical_review_path("./pkg/foo.ts");
    let b = canonical_review_path("pkg/foo.ts");
    assert_eq!(a, b);
    assert_eq!(path_hash_hex(&a), path_hash_hex(&b));
  }
}
