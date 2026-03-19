/// User-facing progress logger.
///
/// Notes:
/// - Messages are intentionally not "debug" logs.
/// - We print to stderr so JSON output stays valid when `--json` is used.
#[derive(Debug, Clone)]
pub struct UiLogger {
  _json_mode: bool,
}

impl UiLogger {
  pub fn new(json_mode: bool) -> Self {
    Self { _json_mode: json_mode }
  }

  pub fn info(&self, msg: impl AsRef<str>) {
    eprintln!("ai-review: {}", msg.as_ref());
  }

  pub fn warn(&self, msg: impl AsRef<str>) {
    eprintln!("ai-review [warning]: {}", msg.as_ref());
  }
}

