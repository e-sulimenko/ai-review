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

2. **Debug вывод (diff + issues):**

```bash
cargo run -- --debug
```

Пример:

```
================ DEBUG AI Code Review ================
File: src/main.rs
--- Diff ---
+fn main() {}
--- Issues ---
Line 1 [warning] Consider adding documentation for main function -> Add /// comments above main
====================================================
```
