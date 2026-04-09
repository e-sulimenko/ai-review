### 1. Компиляция и запуск

1. Скомпилируем проект:

```bash
cargo build --release
```

### 1.1 Установка (чтобы `ai-review` был в PATH)

Скрипт соберёт бинарник в release и установит его так, чтобы можно было вызывать `ai-review` из терминала.

```bash
./install.sh
```

Опционально:

```bash
./install.sh --prefix "$HOME/.local"   # установит в $HOME/.local/bin
./install.sh --force                  # без вопроса перезапишет существующий бинарник
```

2. Запускаем ревью (подкоманда `run`):

```bash
cargo run -- run
```

После `cargo build --release` то же самое из бинарника: `target/release/ai-review run`.
Список всех подкоманд: `cargo run -- --help`.

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
cargo run -- run --json
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
cargo run -- run --md
```

Другие флаги у `run`: `--debug`, `--no-cache`.

---

### 3. Команды CLI

Подкоманда передаётся первым аргументом (после `cargo run --` — сразу имя подкоманды).

| Команда | Назначение |
|--------|------------|
| `run` | Запуск ревью изменений в текущей ветке относительно git. Флаги: `--json`, `--md`, `--debug`, `--no-cache`. |
| `clean-cache` | Удалить каталог кеша ответов LLM: `.ai-review/cache`. |
| `clean-review` | Удалить каталог сохранённых markdown-отчётов: `.ai-review/reviews`. |
| `clean` | Удалить оба каталога — и кеш, и отчёты. |

Примеры:

```bash
cargo run -- clean-cache
cargo run -- clean-review
cargo run -- clean
target/release/ai-review run --no-cache
```

---

### 4. Конфиг LLM
Файл конфигурации: `~/.ai-review/config.json` (или `./.ai-review/config.json`).

В секции `llm` можно указать `extra_body` — любые дополнительные поля, которые будут добавлены в JSON body запроса к LLM на том же уровне, что и `model` и `messages`.

Также в корне конфигурации можно указать фильтрацию файлов:
- `include`: список путей/папок, которые нужно проверять;
- `exclude`: список путей/папок, которые нужно исключить из проверки.

Если указаны оба, сначала остаются только пути из `include`, затем из них убираются совпадения с `exclude`. Можно указать только `include`, только `exclude` или оба.

Также можно настроить поведение ретраев и количество candidate-ревью:
- `max_retry_count` (по умолчанию `3`) — сколько раз повторять запрос, если LLM вернул невалидный JSON.
- `candidate_reviews_per_diff` (по умолчанию `2`) — сколько candidate-ревью сгенерировать на один diff-файл перед дедупликацией.

Пример:
```json
{
  "include": ["src/", "README.md"],
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
