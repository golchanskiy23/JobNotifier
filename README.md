# Job Notifier — инструкция по запуску и проверке

## 1. Сборка

```bash
cargo build --release
```

Бинарник появится в `target/release/JobNotifier`.

---

## 2. Запуск тестов

```bash
cargo test
```

Ожидаемый результат: `test result: ok. 23 passed; 0 failed`.

---

## 3. Конфигурация

Отредактируй `Config.toml`:

```toml
[scraping]
urls = [
  "https://hh.ru/search/vacancy?text=rust",
]
interval_minutes = 1440
timeout_secs = 10
keywords = ["rust", "Go", "backend", "разработчик"]
user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"
```

> Без `keywords` скрейпер не вернёт ни одной вакансии.
> Поиск идёт по границам слова — `"Go"` найдёт `"Middle Go Backend Engineer"`,
> но не зацепит `"Django"` или `"MongoDB"`.

---

## 4. Однократный скрейпинг

```bash
cargo run -- run --run-once
```

Пример вывода:

```
Found 3 new job(s):
==================================================

 Job #1
Title: Middle Go Backend Engineer
URL: https://hh.ru/vacancy/12345678
Grade: Middle
Found: 2026-03-19 10:22:01 UTC
```

---

## 5. Трекер заявок — полный сценарий

### Добавить заявку

```bash
cargo run -- add-application \
  --company "Яндекс" \
  --position "Middle Go Backend Engineer" \
  --reply-days 14 \
  --job-url "https://hh.ru/vacancy/12345678" \
  --notes "Откликнулся через hh.ru"
```

Вывод:

```
Application added:
  ID:       550e8400-e29b-41d4-a716-446655440000
  Company:  Яндекс
  Position: Middle Go Backend Engineer
  Applied:  2026-03-19
  Deadline: 2026-04-02
  Status:   Submitted
  Job URL:  https://hh.ru/vacancy/12345678
  Notes:    Откликнулся через hh.ru
```

Добавь ещё несколько:

```bash
cargo run -- add-application \
  --company "Озон" \
  --position "Senior Rust Engineer" \
  --job-url "https://hh.ru/vacancy/99887766"

cargo run -- add-application \
  --company "Авито" \
  --position "Backend Developer" \
  --reply-days 7
```

### Посмотреть список

```bash
cargo run -- list-applications
```

```
ID                                    Company  Position                    Applied     Deadline    Status         Job URL
------------------------------------  -------  --------------------------  ----------  ----------  -------------  ---------------------------
550e8400-e29b-41d4-a716-446655440000  Яндекс   Middle Go Backend Engineer  2026-03-19  2026-04-02  Submitted      https://hh.ru/vacancy/12345678
...
```

### Обновить статус

```bash
cargo run -- update-status \
  --id 550e8400-e29b-41d4-a716-446655440000 \
  --status in-review
```

Допустимые значения: `submitted`, `in-review`, `rejected`, `offer-received`, `withdrawn`.

Проверка ошибки при неверном статусе:

```bash
cargo run -- update-status \
  --id 550e8400-e29b-41d4-a716-446655440000 \
  --status pending
# Error: unknown status 'pending'. Valid values: submitted, in-review, rejected, offer-received, withdrawn
```

### Удалить заявку

```bash
cargo run -- delete-application --id 550e8400-e29b-41d4-a716-446655440000
# Application '550e8400-...' deleted.
```

---

## 6. Проверка уведомлений о дедлайнах

Добавь заявку с `--reply-days 0` (дедлайн = сегодня):

```bash
cargo run -- add-application \
  --company "Тест Дедлайн" \
  --position "Rust Dev" \
  --reply-days 0
```

Затем запусти скрейпинг:

```bash
cargo run -- run --run-once
```

В выводе появится блок:

```
=== Deadline reminders ===
[!] Тест Дедлайн — Rust Dev (deadline: 2026-03-19)
```

---

## 7. Статистика и история вакансий

```bash
# Общая статистика
cargo run -- run --stats

# Последние 5 найденных вакансий
cargo run -- run --recent 5

# Удалить записи старше 30 дней
cargo run -- run --cleanup 30
```

---

## 8. Фоновый режим через systemd

```bash
sudo cp job-notifier.service /etc/systemd/user/
systemctl --user daemon-reload
systemctl --user start job-notifier
systemctl --user enable job-notifier

# Логи в реальном времени
journalctl --user -u job-notifier -f
```

Планировщик запускается каждые `interval_minutes` минут (по умолчанию 1440 = раз в сутки).

---

## 9. Прямая работа с базой данных

```bash
sqlite3 job_notifier.db
```

```sql
-- Все заявки с URL вакансии
SELECT id, company, position, status, job_url FROM applications;

-- Заявки с дедлайном сегодня
SELECT * FROM applications
WHERE date(applied_at, '+' || expected_reply_days || ' days') = date('now')
  AND status IN ('Submitted', 'InReview');

-- Последние найденные вакансии
SELECT dedup_key, seen_at FROM seen_jobs ORDER BY seen_at DESC LIMIT 10;
```

---

## 10. Дедупликация вакансий

Каждая найденная вакансия сохраняется в `seen_jobs` с ключом `company:title:url`.
При следующем запуске скрейпер проверяет этот ключ — уже виденные вакансии не уведомляют повторно.
