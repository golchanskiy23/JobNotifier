# JobNotifier

Инструмент для мониторинга вакансий с произвольных сайтов и трекинга откликов. Написан на Rust.

## Возможности

- Скрейпинг любых сайтов с вакансиями — без привязки к структуре URL
- Поддержка SPA-сайтов (React/Vue) через headless Chromium
- Фильтрация по ключевым словам с учётом границ слов (`Go` не зацепит `MongoDB`)
- Дедупликация — повторные уведомления по одной вакансии не приходят
- Трекер откликов с дедлайнами и статусами
- Планировщик с запуском через systemd

## Установка

```bash
cargo build --release
```

Для SPA-сайтов дополнительно нужен Chromium:

```bash
sudo pacman -S chromium   # Arch Linux
# или
sudo apt install chromium # Debian/Ubuntu
```

## Конфигурация

`Config.toml`:

```toml
[scraping]
urls = [
  "https://careers.kaspersky.ru/stack/GO",
  "https://hh.ru/search/vacancy?text=golang",
]
interval_minutes = 1440
timeout_secs = 10
keywords = ["Go", "Golang", "Rust"]
user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"

# Для SPA-сайтов (React/Vue) — включить headless Chromium
# use_browser = true
# chrome_path = "/usr/bin/chromium"  # если не находит автоматически
# browser_wait_ms = 3000             # время ожидания JS-рендеринга
```

`keywords` — обязательное поле. Без него скрейпер не вернёт ни одной вакансии.

## Использование

### Однократный запуск

```bash
cargo run -- run --run-once
```

### Планировщик (запускается каждые `interval_minutes` минут)

```bash
cargo run -- run
```

### Статистика и история

```bash
cargo run -- run --stats          # общая статистика
cargo run -- run --recent 10      # последние 10 найденных вакансий
cargo run -- run --cleanup 30     # удалить записи старше 30 дней
```

## Трекер откликов

Заявки добавляются вручную — берёшь URL из уведомления скрейпера и добавляешь отклик.

```bash
# Добавить отклик
cargo run -- add-application \
  --company "Kaspersky" \
  --position "Developer Go (Sandbox)" \
  --job-url "https://careers.kaspersky.ru/vacancy/24936" \
  --reply-days 14 \
  --notes "Откликнулся через сайт"

# Список всех откликов
cargo run -- list-applications

# Обновить статус
cargo run -- update-status --id <UUID> --status in-review

# Удалить
cargo run -- delete-application --id <UUID>
```

Статусы: `submitted`, `in-review`, `rejected`, `offer-received`, `withdrawn`.

При запуске скрейпера выводятся напоминания о заявках с дедлайном сегодня:

```
=== Deadline reminders ===
[!] Kaspersky — Developer Go (Sandbox) (deadline: 2026-04-02)
```

## Systemd

```bash
sudo cp job-notifier.service /etc/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now job-notifier
journalctl --user -u job-notifier -f
```

## Тесты

```bash
cargo test
```

## Архитектура

```
Config.toml
    └── urls + keywords
            │
            ▼
    HttpFetcher / BrowserFetcher (headless Chrome)
            │
            ▼
    extract_jobs_from_html()
    — ищет текстовые элементы с ключевыми словами
    — находит ближайшую ссылку (любую, без URL-паттернов)
    — дедуплицирует по URL
            │
            ▼
    SQLite (seen_jobs)  →  ConsoleNotifier
            │
            ▼
    ApplicationTracker (отклики, дедлайны)
```
