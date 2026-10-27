# MistFences

**Бесплатная замена Stardock Fences для Windows 11 и Windows 10.**
A free Stardock Fences alternative for Windows 11 and Windows 10 — [English below](#english).

Области (группы) на рабочем столе, сворачивание по наведению, вкладки, правила
автосортировки, и значки на самом рабочем столе, которые выглядят и ведут себя как значки
Windows 11.

## Скачать

Готовая сборка — на странице [Releases](../../releases): архив
`MistFences-v<версия>-x64.zip`. Распакуйте его в постоянную папку и запустите
`mistfences.exe`. Подробности — в `README.md` внутри архива
([docs/mistfences/USER-GUIDE.md](docs/mistfences/USER-GUIDE.md)).

Нужна Windows 11 или Windows 10 22H2 (x64) и Microsoft Edge WebView2 Runtime. Программа не
подписана: если SmartScreen предупредит, нажмите «Подробнее» → «Выполнить в любом случае».
Перед запуском закройте Stardock Fences, если он работает.

## Что умеет

- Области со стеклянным фоном: перемещение, изменение размера, сворачивание в заголовок
  и раскрытие при наведении без задержек, вкладки, правила автосортировки, снимки раскладки.
- **Свободные значки на рабочем столе, как в Windows 11:** сетка и начало сетки берутся у
  Проводника, подписи переносятся и обрезаются так же, как у него; выделение рамкой,
  перетаскивание, меню Проводника, F2 с его же полем переименования, Del, Ctrl+C / X / V / A.
- Перетаскивание на папку, ярлык папки или Корзину переносит файлы через файловую операцию
  Windows (с её окнами конфликтов и ошибок); перестановка значков файлы не трогает.
- Рамка **правой** кнопкой по пустому месту выделяет значки и предлагает «Создать область
  здесь», как в Fences.
- Выход возвращает обычные значки Windows; сторожевой процесс вернёт их и при сбое.
- Сетевые службы PecoFence (проверка обновлений, отправка отзывов с журналами, оценка в
  Store) отключены.
- Управление из командной строки: `mistfences-cli.exe` (`describe` выводит каталог команд).

## Сборка из исходников

Нужны Rust (MSVC, версия из `rust-toolchain.toml`) и Visual Studio Build Tools с Windows SDK.

```powershell
./scripts/mistfences/build.ps1 -Release      # target\release
./scripts/mistfences/package.ps1             # dist\MistFences-v<версия>-x64.zip
cargo test --workspace
```

Тесты поведения на чистой Windows запускаются в Windows Sandbox:
`scripts/mistfences/sandbox/run-sandbox-tests.ps1 -AppDir <папка сборки> -Script <скрипт>`
(описание — в [docs/mistfences/FREE-ICONS.md](docs/mistfences/FREE-ICONS.md)).

## Происхождение и лицензия

MistFences — изменённая версия [PecoFence](https://github.com/DayuanJiang/PecoFence)
(основа — коммит `5376f8b` от 18.10.2026) и распространяется на тех же условиях,
[Apache License 2.0](LICENSE). Изменения перечислены в [NOTICE](NOTICE),
[docs/mistfences/CHANGES.md](docs/mistfences/CHANGES.md) и
[docs/mistfences/FREE-ICONS.md](docs/mistfences/FREE-ICONS.md). Остальная документация в
`docs/` написана для PecoFence и описывает общие возможности; имена `pecofence` в ней и в
именах внутренних пакетов Cargo остались от исходного проекта.

MistFences не связан со Stardock; Fences — товарный знак Stardock Corporation.

---

## English

MistFences is a modified version of [PecoFence](https://github.com/DayuanJiang/PecoFence)
(Apache-2.0): desktop fences with roll-up on hover, tabs, sorting rules and snapshots, plus
free icons on the desktop itself that look and behave like Windows 11's own — Explorer's
grid and label layout, rubber-band selection, Explorer's context menu and rename box,
Delete / F2 / Ctrl+C / X / V / A, drops onto folders through the Windows file operation,
and a right-button rectangle that offers a new fence (as in Fences). PecoFence's network
services (update check, feedback upload, Store rating) are switched off.

Download the ZIP from [Releases](../../releases), extract it to a permanent folder and run
`mistfences.exe` (Windows 11 or Windows 10 22H2 x64, WebView2 Runtime). The program is not
code-signed; SmartScreen may ask for confirmation. Close Stardock Fences first if it runs.
Exit from the tray menu gives Windows its desktop icons back.

Build: `./scripts/mistfences/build.ps1 -Release`, package:
`./scripts/mistfences/package.ps1`, tests: `cargo test --workspace`.

Not affiliated with Stardock; Fences is a trademark of Stardock Corporation.
