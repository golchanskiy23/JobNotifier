# JobNotifier

Инструмент для мониторинга вакансий с произвольных сайтов и трекинга откликов.

## Возможности

- Скрейпинг любых сайтов — без привязки к структуре URL или API
- Поддержка SPA-сайтов (React/Vue) через headless Chromium, остальные — через HTTP
- Per-URL выбор скрейпера через `browser_urls` в конфиге
- Фильтрация по ключевым словам с учётом границ слов (`Go` не зацепит `MongoDB`)
- Автоматическое определение компании по домену вакансии
- Дедупликация по URL — повторные уведомления не приходят
- Трекер откликов с дедлайнами и статусами

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

Для SPA-сайтов нужен Chromium:

```bash
sudo pacman -S chromium   # Arch Linux
sudo apt install chromium # Debian/Ubuntu
```

## Конфигурация

`Config.toml`:

```toml
[scraping]
# список сайтов с вакансиями
urls = [
  "https://careers.kaspersky.ru/stack/GO",
  "https://career.avito.com/vacancies/?q=&action=filter",
]
# интервал автоматического скрейпинга
interval_minutes = 1440
timeout_secs = 10
# ключевые слова выборки
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

### Тесты

```bash
cargo test
```

### Обязательно перед запуском
```bash
touch job_notifier.db   # создать файл БД перед первым запуском
# Docker
docker compose up -d --build # сборка контейнера
```

### Однократный запуск

```bash
cargo build --release
cargo run -- run --run-once
# Docker:
docker compose exec job-notifier ./JobNotifier run --run-once
```

### Планировщик

```bash
cargo run -- run
# Docker: планировщик запускается автоматически при docker compose up
docker compose up -d --build
```

### История и статистика

```bash
cargo run -- run --stats        # общая статистика
cargo run -- run --recent 20    # последние 20 найденных вакансий
# Docker:
docker compose exec job-notifier ./JobNotifier run --stats
docker compose exec job-notifier ./JobNotifier run --recent 20
```

### Трекер откликов

```bash
# Добавить отклик
cargo run -- add-application \
  --company "Kaspersky" \
  --position "Developer Go (Sandbox)" \
  --job-url "https://careers.kaspersky.ru/vacancy/24936" \
  --reply-days 14
# Docker:
docker compose exec job-notifier ./JobNotifier add-application \
  --company "Kaspersky" \
  --position "Developer Go (Sandbox)" \
  --job-url "https://careers.kaspersky.ru/vacancy/24936" \
  --reply-days 14

# Список откликов
cargo run -- list-applications
# Docker:
docker compose exec job-notifier ./JobNotifier list-applications

# Обновить статус
cargo run -- update-status --id <UUID> --status in-review
# Docker:
docker compose exec job-notifier ./JobNotifier update-status --id <UUID> --status in-review

# Удалить
cargo run -- delete-application --id <UUID>
# Docker:
docker compose exec job-notifier ./JobNotifier delete-application --id <UUID>
```

Статусы: `submitted`, `in-review`, `rejected`, `offer-received`, `withdrawn`.

При запуске выводятся напоминания о заявках с дедлайном сегодня:

```
=== Deadline reminders ===
[!] Kaspersky — Developer Go (Sandbox) (deadline: 2026-04-02)
```

### БД

Вакансии хранятся в SQLite (`job_notifier.db`). Просмотр:

```bash
sqlite3 job_notifier.db "SELECT title, company, url FROM seen_jobs ORDER BY id DESC LIMIT 50;"
# Docker:
docker compose exec job-notifier sqlite3 job_notifier.db "SELECT title, company, url FROM seen_jobs ORDER BY id DESC LIMIT 50;"
```

Очистить:

```bash
sqlite3 job_notifier.db "DELETE FROM seen_jobs;"
# Docker:
docker compose exec job-notifier sqlite3 job_notifier.db "DELETE FROM seen_jobs;"
```

## Получение логов и остановка контейнера

```bash
# Логи (Ctrl+C отсоединяет от вывода, контейнер продолжает работать)
docker compose logs -f

# Остановить
docker compose down
```

`Config.toml` монтируется из корня репозитория как read-only — отредактируй его перед запуском:
- `urls` / `browser_urls` — список сайтов
- `interval_minutes` — интервал прогона
- `keywords` — ключевые слова фильтрации

Chromium уже включён в образ, `chrome_path` в конфиге указывать не нужно.

## Systemd

Альтернатива Docker — запуск как пользовательский systemd-сервис (без root). Сервис стартует при входе пользователя и перезапускается при падении.

### 1. Сборка бинарника

```bash
cargo build --release
```

### 2. Установка бинарника

Скопируй бинарник в удобное место. Вариант — рядом с проектом:

```bash
# Либо оставь в target/release и укажи полный путь в service-файле
# Либо скопируй в ~/.local/bin (должен быть в PATH)
cp target/release/JobNotifier ~/.local/bin/JobNotifier
```

### 3. Настройка service-файла

Открой `job-notifier.service` и укажи путь к директории проекта в `WorkingDirectory` — там должны лежать `Config.toml` и `job_notifier.db`:

### 4. Установка сервиса

```bash
mkdir -p ~/.config/systemd/user
cp job-notifier.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now job-notifier
```

### 5. Управление

```bash
systemctl --user status job-notifier      # статус
systemctl --user stop job-notifier        # остановить
systemctl --user restart job-notifier     # перезапустить
systemctl --user disable job-notifier     # убрать из автозапуска
```

### 6. Логи

```bash
journalctl --user -u job-notifier -f      # следить в реальном времени
journalctl --user -u job-notifier -n 100  # последние 100 строк
journalctl --user -u job-notifier --since "1 hour ago"
```

### 7. Автозапуск без активной сессии

По умолчанию пользовательские сервисы останавливаются при выходе из системы. Чтобы сервис работал в фоне постоянно:

```bash
loginctl enable-linger $USER
```

