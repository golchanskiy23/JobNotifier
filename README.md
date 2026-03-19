# JobNotifier

Инструмент для мониторинга вакансий с произвольных сайтов и трекинга откликов. Написан на Rust.

## Возможности

- Скрейпинг любых сайтов — без привязки к структуре URL или API
- Поддержка SPA-сайтов (React/Vue) через headless Chromium, остальные — через HTTP
- Per-URL выбор скрейпера через `browser_urls` в конфиге
- Фильтрация по ключевым словам с учётом границ слов (`Go` не зацепит `MongoDB`)
- Автоматическое определение компании по домену вакансии
- Дедупликация по URL — повторные уведомления не приходят
- Трекер откликов с дедлайнами и статусами

## Установка

```bash
cargo build --release
```

Для SPA-сайтов нужен Chromium:

```bash
sudo pacman -S chromium   # Arch Linux
sudo apt install chromium # Debian/Ubuntu
```

## Конфигурация

`Config.toml`:

```toml
[scraping]
urls = [
  "https://careers.kaspersky.ru/stack/GO",
  "https://career.avito.com/vacancies/?q=&action=filter",
]
interval_minutes = 1440
timeout_secs = 10
keywords = ["Go", "Golang"]
user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"

# SPA-сайты — скрейпятся через headless Chromium
# Остальные URL из списка urls идут через обычный HTTP
browser_urls = [
  "https://careers.kaspersky.ru/",
  "https://internship.vk.company/",
]

# chrome_path = "/usr/bin/chromium"  # если не находит автоматически
browser_wait_ms = 5000               # время ожидания JS-рендеринга (мс)

# Маппинг домен → название компании
[companies]
"careers.kaspersky.ru" = "Kaspersky"
"career.avito.com" = "Avito"
"internship.vk.company" = "VK"
```

`keywords` — обязательное поле. Без него скрейпер не вернёт ни одной вакансии.

Если домен не указан в `[companies]` — название берётся автоматически из второго уровня домена (например `Kaspersky` из `careers.kaspersky.ru`).

## Использование

### Однократный запуск

```bash
cargo run -- run --run-once
```

### Планировщик

```bash
cargo run -- run
```

### История и статистика

```bash
cargo run -- run --stats        # общая статистика
cargo run -- run --recent 20    # последние 20 найденных вакансий
```

## Трекер откликов

```bash
# Добавить отклик
cargo run -- add-application \
  --company "Kaspersky" \
  --position "Developer Go (Sandbox)" \
  --job-url "https://careers.kaspersky.ru/vacancy/24936" \
  --reply-days 14

# Список откликов
cargo run -- list-applications

# Обновить статус
cargo run -- update-status --id <UUID> --status in-review

# Удалить
cargo run -- delete-application --id <UUID>
```

Статусы: `submitted`, `in-review`, `rejected`, `offer-received`, `withdrawn`.

При запуске выводятся напоминания о заявках с дедлайном сегодня:

```
=== Deadline reminders ===
[!] Kaspersky — Developer Go (Sandbox) (deadline: 2026-04-02)
```

## БД

Вакансии хранятся в SQLite (`job_notifier.db`). Просмотр:

```bash
sqlite3 job_notifier.db "SELECT title, company, url FROM seen_jobs ORDER BY id DESC LIMIT 50;"
```

Очистить:

```bash
sqlite3 job_notifier.db "DELETE FROM seen_jobs;"
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
    └── urls + keywords + browser_urls + companies
            │
            ├── browser_urls → BrowserScraper (headless Chromium)
            └── остальные   → UniversalScraper (HTTP)
                    │
                    ▼
            extract_jobs_from_html / JS
            — ищет элементы с ключевыми словами
            — находит ближайшую ссылку (без URL-паттернов)
            — определяет компанию по домену
                    │
                    ▼
            SQLite seen_jobs (id, title, company, url)
                    │
                    ▼
            ConsoleNotifier + ApplicationTracker
```
