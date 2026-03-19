# Tasks: job-notifier-enhanced

## Task List

- [x] 1. Подготовка инфраструктуры
  - [x] 1.1 Добавить `proptest = "1"` в `[dev-dependencies]` в `Cargo.toml`
  - [x] 1.2 Добавить `StorageError::NotFound` в `src/errors.rs`
  - [x] 1.3 Создать `src/domain/application.rs` с типами `ApplicationStatus`, `Application`, `AddApplicationCmd`
  - [x] 1.4 Обновить `src/domain.rs` — добавить `pub mod application; pub use application::{Application, ApplicationStatus, AddApplicationCmd};`

- [x] 2. Расширение конфига
  - [x] 2.1 Добавить поля `keywords: Vec<String>` и `user_agent: Option<String>` в `ScrapingConfig` (`src/config.rs`) с `#[serde(default)]`

- [x] 3. UniversalScraper
  - [x] 3.1 Создать `src/scraper/grade.rs` — вынести `detect_grade` из `HhScraper` в отдельную pub-функцию
  - [x] 3.2 Создать `src/scraper/universal.rs` — реализовать `UniversalScraper` с методами `new`, `extract_jobs`, `resolve_url` и impl `Scraper`
  - [x] 3.3 Обновить `src/scraper/mod.rs` — добавить `pub mod universal; pub mod grade; pub use universal::UniversalScraper;`
  - [x] 3.4 Обновить `HhScraper` в `src/scraper/hh.rs` — использовать `grade::detect_grade` вместо собственного метода

- [x] 4. Расширение Storage для Application
  - [x] 4.1 Добавить методы `add_application`, `list_applications`, `update_application_status`, `delete_application`, `get_deadline_applications` в трейт `Storage` (`src/storage/mod.rs`)
  - [x] 4.2 Реализовать новые методы в `SqliteStorage` (`src/storage/sqlite.rs`) — добавить миграцию таблицы `applications`

- [x] 5. ApplicationTracker
  - [x] 5.1 Создать `src/tracker/mod.rs` — реализовать `ApplicationTracker` с методами `add`, `list`, `update_status`, `delete`, `get_due_today`
  - [x] 5.2 Обновить `src/main.rs` — добавить `mod tracker;`

- [x] 6. Расширение Notifier
  - [x] 6.1 Добавить метод `notify_deadlines(&self, apps: &[Application]) -> Result<(), NotifierError>` в трейт `Notifier` (`src/notifier/mod.rs`)
  - [x] 6.2 Реализовать `notify_deadlines` в `ConsoleNotifier` (`src/notifier/console.rs`)

- [x] 7. Расширение Scheduler
  - [x] 7.1 Обновить `JobScheduler::run_once` в `src/scheduler.rs` — добавить вызов `get_deadline_applications` и `notify_deadlines` после скрейпинга

- [x] 8. CLI-команды для управления заявками
  - [x] 8.1 Обновить `src/main.rs` — заменить плоский `CliArgs` на subcommands (`clap`): `run`, `add-application`, `list-applications`, `update-status`, `delete-application`, плюс сохранить флаги `--stats`, `--recent`, `--cleanup` в подкоманде `run`
  - [x] 8.2 Реализовать обработчики CLI-команд в `src/main.rs` — `handle_add_application`, `handle_list_applications`, `handle_update_status`, `handle_delete_application`

- [x] 9. Обновление `main.rs` для UniversalScraper
  - [x] 9.1 Заменить `HhScraper` на `UniversalScraper` в `main.rs`, передавая `keywords` и `user_agent` из конфига

- [x] 10. Тесты
  - [x] 10.1 Написать unit-тесты для `UniversalScraper::extract_jobs` (пустой HTML, HTML без совпадений, HTML с совпадениями)
  - [x] 10.2 Написать unit-тест для `AppConfig` без поля `keywords` → `keywords == []`
  - [x] 10.3 Написать unit-тесты для `ApplicationStatus::from_str_cli` (допустимые и недопустимые значения)
  - [x] 10.4 Написать unit-тест для `Application::is_due_today` с терминальным статусом
  - [x] 10.5 Написать unit-тест для `ConsoleNotifier::notify_deadlines` vs `notify` (разный вывод)
  - [x] 10.6 Написать property-тест P1: фильтрация div по ключевым словам
  - [x] 10.7 Написать property-тест P2: извлечение полей из совпадающего div
  - [x] 10.8 Написать property-тест P3: преобразование относительного URL в абсолютный
  - [x] 10.9 Написать property-тест P4: определение грейда из заголовка
  - [x] 10.10 Написать property-тест P5: десериализация keywords из TOML
  - [x] 10.11 Написать property-тест P6: round-trip добавления Application (in-memory SQLite)
  - [x] 10.12 Написать property-тест P7: сортировка списка заявок
  - [x] 10.13 Написать property-тест P8: обновление статуса заявки
  - [x] 10.14 Написать property-тест P9: удаление заявки
  - [x] 10.15 Написать property-тест P10: получение заявок с дедлайном сегодня (исключая терминальные статусы)
  - [x] 10.16 Написать property-тест P11: создание заявки с правильными полями
  - [x] 10.17 Написать property-тест P12: форматирование строки таблицы заявок
  - [x] 10.18 Написать property-тест P13: уведомление о дедлайне содержит обязательные поля
