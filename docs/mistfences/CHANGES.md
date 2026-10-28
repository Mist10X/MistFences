# MistFences: изменения относительно PecoFence

MistFences — модификация [PecoFence](https://github.com/DayuanJiang/PecoFence) (Apache-2.0).
Основа — коммит `5376f8bd82b474c67b08037de5e9cfbd138400a9` от 18.10.2026 (тег
`upstream-base-2026-10-18`, удалённый репозиторий `upstream`, отправка в него отключена).
Работа ведётся в ветке `mistfences`. Полный список правок: `git diff upstream-base-2026-10-18`.

## Этап 1

### Собственная идентичность (`crates/core/src/brand.rs` и места использования)

| Что | PecoFence | MistFences |
|---|---|---|
| Исполняемые файлы | `pecofence.exe`, `pecofence-watchdog.exe`, `pecofence-cli.exe` | `mistfences.exe`, `mistfences-watchdog.exe`, `mistfences-cli.exe` |
| Мьютекс одиночного экземпляра | `Local\PecoFence.SingleInstance`, `Local\openFence.SingleInstance` | `Local\MistFences.SingleInstance`, `Local\MistFences.SingleInstance.Compat` |
| Канал CLI | `\\.\pipe\PecoFence.<SID>[.<экземпляр>]` | `\\.\pipe\MistFences.<SID>[.<экземпляр>]` |
| Конфигурация | `%APPDATA%\PecoFence` (с подхватом `OpenFence`) | `%APPDATA%\MistFences`, чужие папки не подхватываются |
| Журналы, дампы, маркер значков, профиль WebView2 | `%LOCALAPPDATA%\PecoFence`, `pecofence*.log` | `%LOCALAPPDATA%\MistFences`, `mistfences*.log` |
| Переменные окружения | `PECOFENCE_*`, запасной вариант `OPENFENCE_*` | только `MISTFENCES_*` |
| Значение автозапуска (HKCU Run) | `PecoFence`, удаление `openFence` | `MistFences`, удаление только `MistFences.Legacy` |
| Классы окон | `PecoFence.*` | `MistFences.*` |
| Ресурс версии exe | ProductName PecoFence | ProductName MistFences, авторство PecoFence сохранено |

Пакеты Cargo, крейты (`pecofence_core`, `pecofence_platform` и т. д.) и внутренние типы
сохранили свои имена: это упрощает перенос изменений из upstream.

Отключены сетевые службы upstream, принадлежащие PecoFence: проверка и загрузка обновлений,
отправка отзывов с журналами, оценка в Microsoft Store (`brand::REMOTE_SERVICES = false`).
Видимое имя в переводах интерфейса, всплывающих уведомлениях и окне настроек — MistFences.

### Безопасный предпросмотр (`--mistfences-preview`)

* `crates/platform/src/safety.rs` — блокировка процесса. Включается один раз, до создания
  окон, и не выключается. Проверки встроены в функции платформы, которые меняют систему,
  поэтому действуют при любом источнике вызова (меню, CLI, настройки, горячие клавиши,
  тестовые скрипты):
  * переключение значков рабочего стола (`shell_icons.rs`), запись и удаление в HKCU Run
    (`autostart.rs`) — отказ;
  * контекстное меню Проводника (`shell_menu.rs`), передача сброса папке Проводника
    (`dragdrop.rs`) — отказ;
  * файловые операции (`shell.rs`: перемещение, копирование, переименование, ярлыки) —
    только внутри песочницы; `ShellExecute` — только для элементов песочницы и только с
    глаголами open/properties;
  * перетаскивание наружу (`dragdrop.rs`) — отпускание над чужим окном отменяет сброс;
    приём перетаскивания извне (`fence_window/dnd.rs`) — отключён;
  * глобальные сочетания клавиш (`hotkey.rs`) — только Esc для отмены перемещения группы;
  * перехват мыши на рабочем столе (`rawinput.rs`), регистрация уведомлений оболочки
    (`shell_notify.rs`), перестановка окна рабочего стола Проводника (`desktop.rs`) — отказ;
  * папка рабочего стола (`shell::user_desktop`) подменяется песочницей; общий рабочий стол
    и системные значки (Корзина, «Этот компьютер») не читаются.
* `crates/app/src/preview.rs` — папка предпросмотра (песочница с примерами, конфигурация,
  тема «MistFences Dark»), принудительно безопасные настройки, стартовая раскладка из трёх
  свёрнутых групп «Игры», «Программы», «Файлы» с правилами распределения.
* `crates/app/src/main.rs` — разбор `--mistfences-preview` и `--preview-root`, отдельный
  экземпляр `preview` (свой мьютекс, канал, журналы), включение блокировки до создания окон.
* `crates/app/src/state.rs` — отдельная папка конфигурации предпросмотра.
* `crates/app/src/anchor.rs` — в предпросмотре значки не скрываются, маркер не пишется.
* `crates/watchdog/src/main.rs` — ничего не делает для маркера предпросмотра.
* Экспорт конфигурации в предпросмотре — только в папку предпросмотра.
* Запись `.url`, создание файлов и папок, файловые команды тестовых скриптов — через ту же
  проверку песочницы.

### CLI

* `mistfences-cli quit` (IPC-метод `app.quit`) — штатно закрывает экземпляр, как пункт
  «Выход» в меню трея.

### Скрипты (`scripts/mistfences/`)

`build.ps1`, `run-preview.ps1`, `stop-preview.ps1`, `measure-preview.ps1`,
`probe-safety.ps1`, `snapshot-system.ps1`, `compare-snapshots.ps1` — см. `PREVIEW.md`.

### Тесты

Обновлены ожидания, в которых проверялись старые имена (`brand.rs`, CLI `local.rs`,
`share_log.rs` — имя модуля приложения теперь `mistfences::`). Добавлены тесты
`safety.rs` (границы песочницы) и `preview.rs` (примеры, настройки, стартовая раскладка).

## Установщик и исправления

### Установщик (`packaging/inno/mistfences.iss`)

Сценарий upstream `pecofence.iss` ссылался на `pecofence.exe`, идентификатор
`DayuanJiang.PecoFence`, мьютексы и значения автозапуска PecoFence: собранный по нему
установщик не нашёл бы файлов MistFences, а с подставленными файлами записался бы в «Установленные
приложения» как PecoFence и удалял бы чужую запись автозапуска. Новый сценарий:

* идентичность MistFences (`Mist10X.MistFences`, `%LOCALAPPDATA%\Programs\MistFences`, значения
  `MistFences` и `MistFences.Legacy` в `HKCU\…\Run`, мьютексы `Local\MistFences.SingleInstance*`);
* установка для текущего пользователя без прав администратора, 10 языков (первым — русский);
* работающий MistFences закрывается штатно (`mistfences-watchdog --restore --quiet`, как «Выйти»
  в трее — значки Windows возвращаются), никогда не завершается принудительно; тихое обновление
  запускает его снова, неудачное — возвращает прежнюю копию;
* ярлыки «Пуска»: MistFences, «Восстановить рабочий стол Windows» (`--restore`), руководство;
* WebView2 Runtime: если его нет, установщик предлагает скачать и поставить загрузчик Microsoft
  (сбой загрузки только предупреждает — MistFences работает и без него);
* удаление закрывает MistFences, снимает только свою запись автозапуска (в том числе с
  `--portable`) и по запросу (`/REMOVEDATA=1` в тихом режиме) удаляет настройки и журналы;
* сохранены проверки upstream: не ставить в чужую непустую папку, не переносить установку,
  не понижать версию, принимать папку распакованного ZIP.

`scripts/mistfences/package.ps1` собирает установщик и ZIP из одних и тех же файлов
(`-Format All|Zip|Installer`, `-Iscc`); в ZIP добавлен `restore-desktop.cmd`.
`scripts/make-windows.ps1` и `make-portable.ps1` перенаправляют на него.
`scripts/test-windows-packaging.py` проверяет установщик MistFences под тестовым именем.
`.github/workflows/release.yml` собирает и публикует релиз по тегу `v<версия>`.

### Исправления

* Отзыв: служба отзывов PecoFence отключена (`brand::REMOTE_SERVICES = false`), но пункт трея
  «Отправить отзыв…» и форма в «О программе» оставались — кнопка «Отправить» навсегда отвечала
  «журнал ещё готовится». Теперь вместо формы — «Сообщить о проблеме на GitHub» (страница
  Issues), пункт трея открывает ту же страницу.
* Сообщение о работающем Stardock Fences ссылалось на `docs\mistfences\SWITCH.md`, которого
  нет в публичной версии; теперь — на раздел руководства рядом с программой.
* `build.ps1` собирает приложение и CLI отдельно: при общей сборке Cargo включал функцию
  `describe` (schemars) в общих крейтах и тянул её в `mistfences.exe`.

### Документация

`docs/mistfences/USER-GUIDE.md` переписано как полное руководство (установка, области, значки,
вкладки, правила, «Поверх окон», настройки, снимки, переход со Stardock Fences и импорт его
раскладки, CLI, восстановление, обновление и удаление, расположение данных).
