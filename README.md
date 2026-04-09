### Мотивация

Во многих корпоративных продуктах нет встроенных инструментов LLM-ревью (по типу GitLab Duo), а использовать публичные облачные модели нельзя. При этом часто доступны только **внутренние/закрытые LLM** или прокси с OpenAI-совместимым API.

`ai-review` — небольшой CLI, который:
- берёт текущие изменения из git,
- отправляет диффы в LLM,
- печатает отчёт в консоль и/или сохраняет markdown-репорт,
- умеет кешировать ответы и чистить артефакты.

---

### Быстрый старт

#### Требования

Нужен Rust toolchain (Cargo). Если Rust ещё не установлен, самый простой способ — `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

После установки перезапустите терминал и проверьте:

```bash
rustc --version
cargo --version
```

#### Вариант A: запуск без установки (из исходников)

```bash
cargo run -- init
cargo run -- run
```

#### Вариант B: запуск после установки (команда `ai-review` в PATH)

```bash
./install.sh
ai-review init
ai-review run
```

Подсказка: полный список команд — `ai-review --help` (или `cargo run -- --help`).

---

### Dev и Prod режимы

#### Dev (быстро править и запускать из исходников)

```bash
cargo run -- init
cargo run -- run --debug
```

#### Prod (релизная сборка)

```bash
cargo build --release
./target/release/ai-review --help
./target/release/ai-review run
```

---

### Установка и удаление

#### Установка (чтобы `ai-review` был в PATH)

Скрипт соберёт бинарник в release и установит его так, чтобы можно было вызывать `ai-review` из терминала.

Что делает `./install.sh`:
- проверяет наличие `cargo`; если Rust не установлен — предложит поставить его через `rustup` и затем продолжит
- запускает `cargo build --release`
- копирует `target/release/ai-review` в каталог для установки:
  - по умолчанию `/usr/local/bin` (если доступно на запись), иначе `~/.local/bin`
  - либо в `<prefix>/bin`, если указан `--prefix <prefix>`
- если каталог установки не в `PATH`, подсказывает строку для добавления в профиль шелла (например `~/.zshrc`)

```bash
./install.sh
```

Опционально:

```bash
./install.sh --prefix "$HOME/.local"   # установит в $HOME/.local/bin
./install.sh --force                  # без вопроса перезапишет существующий бинарник
```

Если выбранный каталог не в `PATH`, скрипт выведет строку, которую нужно добавить в профиль шелла (например, `~/.zshrc`).

#### Удаление

```bash
./uninstall.sh
```

Опционально:

```bash
./uninstall.sh --prefix "$HOME/.local"
./uninstall.sh --force
```

---

### Конфигурация (`config.json`)

Поддерживаются **два** конфига (они **сливаются**):
- **глобальный**: `~/.ai-review/config.json`
- **локальный** (в текущей директории проекта): `.ai-review/config.json`

Если указаны оба, локальный **перекрывает** глобальный; вложенные объекты объединяются рекурсивно (как `git config`).

#### Минимальный конфиг

```json
{
  "llm": {
    "api_url": "https://openrouter.ai/api/v1/chat/completions",
    "api_key": "YOUR_API_KEY",
    "model": "openrouter/auto"
  }
}
```

#### Поля конфига

- **`llm.api_url`** *(string, required)*: OpenAI-совместимый endpoint chat-completions.
- **`llm.api_key`** *(string, required)*: ключ доступа. Хранится **в открытом виде** в файле.
- **`llm.model`** *(string, required)*: идентификатор модели (формат зависит от вашего провайдера/прокси).
- **`llm.max_retry_count`** *(number, optional, default=3)*: сколько раз повторять запрос, если LLM вернул невалидный JSON.
- **`llm.candidate_reviews_per_diff`** *(number, optional, default=2)*: сколько «кандидатных» ревью генерировать на один diff-файл перед дедупликацией.
- **`llm.extra_body`** *(object, optional)*: дополнительные поля, которые будут добавлены в JSON body запроса к LLM **на одном уровне** с `model` и `messages` (например `temperature`, `max_tokens`).
- **`include`** *(array[string], optional)*: список путей/папок, которые нужно проверять.
- **`exclude`** *(array[string], optional)*: список путей/папок, которые нужно исключить.

Семантика фильтров:
- если указаны **оба**, сначала применяется `include`, потом из результата выкидывается `exclude`
- можно указывать и файлы, и директории; поддерживаются варианты `src` и `src/`

#### Пример расширенного конфига

```json
{
  "include": ["src/", "README.md"],
  "exclude": ["vendor/"],
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

---

### Команды

Подкоманда передаётся первым аргументом.
Если используете `cargo run`, синтаксис такой: `cargo run -- <команда> [флаги]`.

- **`ai-review init`** — создать `config.json` (интерактивно).
  - **`--global`**: записать в `~/.ai-review/config.json` (иначе — `.ai-review/config.json` в текущей директории).
  - **`--yes`**: без вопросов записать шаблон (как `eslint init --yes`).

- **`ai-review run`** — запустить ревью текущих изменений git.
  - **`--json`**: вывести результат в JSON.
  - **`--md`**: сохранить markdown-отчёт в `.ai-review/reviews` (и напечатать путь).
  - **`--debug`**: подробные диагностические логи.
  - **`--no-cache`**: не читать и не писать кеш ревью.

- **`ai-review clean-cache`** — удалить кеш LLM-ответов: `.ai-review/cache`.

- **`ai-review clean-review`** — удалить сохранённые markdown-отчёты: `.ai-review/reviews`.

- **`ai-review clean`** — удалить и кеш, и отчёты.
