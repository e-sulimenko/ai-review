/// User-facing progress logger.
///
/// Notes:
/// - Messages are intentionally not "debug" logs unless `--debug` is set.
/// - We print to stderr so JSON output stays valid when `--json` is used.
use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Debug)]
pub struct UiLogger {
  _json_mode: bool,
  debug_mode: bool,
  start_instant: std::time::Instant,
}

impl UiLogger {
  pub fn new(json_mode: bool, debug_mode: bool) -> Self {
    let start_instant = std::time::Instant::now();
    Self { _json_mode: json_mode, debug_mode, start_instant }
  }

  pub fn debug_enabled(&self) -> bool {
    self.debug_mode
  }

  fn now_unix_millis() -> u128 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_millis()
  }

  fn elapsed_ms(&self) -> u128 {
    self.start_instant.elapsed().as_millis()
  }

  fn prefix(&self, tag: &str) -> String {
    let wall_ms = Self::now_unix_millis();
    let elapsed_ms = self.elapsed_ms();
    format!("ai-review [{} +{}ms]{}", wall_ms, elapsed_ms, tag)
  }

  pub fn info(&self, msg: impl AsRef<str>) {
    eprintln!("{}: {}", self.prefix(""), msg.as_ref());
  }

  pub fn warn(&self, msg: impl AsRef<str>) {
    eprintln!("{} [warning]: {}", self.prefix(""), msg.as_ref());
  }

  pub fn debug(&self, msg: impl AsRef<str>) {
    if self.debug_mode {
      eprintln!("{} [debug]: {}", self.prefix(""), msg.as_ref());
    }
  }
}

