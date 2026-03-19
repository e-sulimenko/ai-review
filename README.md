### 1. Компиляция и запуск

1. Скомпилируем проект:

```bash
cargo build --release
```

2. Запускаем MVP:

```bash
cargo run
```

* По умолчанию вывод будет **человекочитаемый**:

```
================ AI Code Review ================
Files changed: 3
Lines changed: 45
Issues found: 2
Lines to fix: 2
================================================

src/main.rs
--------------------
Line 1 [warning] Consider adding documentation for main function
Suggestion: Add /// comments above main
```

---

### 2. Дополнительные флаги

1. **JSON вывод:**

```bash
cargo run -- --json
```

Пример:

```json
{
  "total_lines": 45,
  "issues": 2,
  "lines_to_fix": 2,
  "files": [
    {
      "path": "src/main.rs",
      "issues": [
        {
          "line": 1,
          "severity": "warning",
          "issue_type": "style",
          "message": "Consider adding documentation for main function",
          "suggestion": "Add /// comments above main"
        }
      ]
    }
  ]
}
```

2. **Markdown отчет:**

```bash
cargo run -- --md
```

---
### 3. Конфиг LLM
Файл конфигурации: `~/.ai-review/config.json` (или `./.ai-review/config.json`).

В секции `llm` можно указать `extra_body` — любые дополнительные поля, которые будут добавлены в JSON body запроса к LLM на том же уровне, что и `model` и `messages`.

Также можно настроить поведение ретраев и количество candidate-ревью:
- `max_retry_count` (по умолчанию `3`) — сколько раз повторять запрос, если LLM вернул невалидный JSON.
- `candidate_reviews_per_diff` (по умолчанию `2`) — сколько candidate-ревью сгенерировать на один diff-файл перед дедупликацией.

Пример:
```json
{
  "llm": {
    "api_url": "https://openrouter.ai/api/v1/chat/completions",
    "api_key": "YOUR_API_KEY",
    "model": "openrouter/hunter-alpha",
    "max_retry_count": 3,
    "candidate_reviews_per_diff": 2,
    "extra_body": {
      "temperature": 0.2,
      "max_tokens": 1200
    }
  }
}
```
