# MistFences — руководство пользователя

Бесплатная замена Stardock Fences для Windows 11 и Windows 10.
A free Stardock Fences alternative for Windows 11 and Windows 10 — [English below](#english).

MistFences раскладывает значки рабочего стола по **областям** (полупрозрачным группам с
заголовком), сворачивает их в заголовок, сам сортирует новые файлы по правилам и рисует
оставшиеся значки прямо на рабочем столе — так же, как это делает Windows. Файлы при этом
никуда не перемещаются: области только показывают то, что лежит на рабочем столе.

**Содержание**

1. [Требования](#1-требования)
2. [Установка](#2-установка)
3. [Первый запуск](#3-первый-запуск)
4. [Области](#4-области)
5. [Значки](#5-значки)
6. [Вкладки](#6-вкладки)
7. [Папка на рабочем столе](#7-папка-на-рабочем-столе)
8. [Правила сортировки](#8-правила-сортировки)
9. [Поверх окон и быстрое скрытие](#9-поверх-окон-и-быстрое-скрытие)
10. [Настройки](#10-настройки)
11. [Снимки, резервные копии, перенос на другой ПК](#11-снимки-резервные-копии-перенос-на-другой-пк)
12. [Переход со Stardock Fences](#12-переход-со-stardock-fences)
13. [Командная строка](#13-командная-строка)
14. [Если что-то пошло не так](#14-если-что-то-пошло-не-так)
15. [Обновление и удаление](#15-обновление-и-удаление)
16. [Где хранятся данные](#16-где-хранятся-данные)

---

## 1. Требования

- Windows 11 или Windows 10 версии 22H2, 64-разрядная.
- Microsoft Edge WebView2 Runtime — нужен только окну настроек. В Windows 11 он уже есть;
  в Windows 10 установщик предложит скачать его сам.
- Права администратора не нужны: программа ставится только для текущего пользователя.

Программа не подписана цифровой подписью, поэтому при первом запуске Windows SmartScreen
может показать предупреждение: нажмите **«Подробнее» → «Выполнить в любом случае»**.

## 2. Установка

Скачайте файлы со страницы [Releases](https://github.com/Mist10X/MistFences/releases).
Рядом с каждым файлом лежит `.sha256` — контрольная сумма для проверки скачанного
(`Get-FileHash <файл>` в PowerShell).

### Установщик (рекомендуется)

1. Если работает Stardock Fences — закройте его (см. [раздел 12](#12-переход-со-stardock-fences)).
2. Запустите `MistFences-v<версия>-x64-setup.exe`.
3. Выберите папку (по умолчанию `%LOCALAPPDATA%\Programs\MistFences`) и, если нужно, ярлык на
   рабочем столе. Если WebView2 Runtime не найден, будет отмечен пункт «Скачать и установить
   Microsoft Edge WebView2 Runtime».
4. На последней странице оставьте «Запустить MistFences».

Установщик создаёт в меню «Пуск» папку **MistFences** с тремя ярлыками:

| Ярлык | Что делает |
|---|---|
| MistFences | запускает программу (повторный запуск открывает настройки уже работающей копии) |
| Восстановить рабочий стол Windows | закрывает MistFences и возвращает обычные значки Windows — аварийный выход |
| Руководство MistFences | открывает это руководство |

Программа появляется в «Параметры → Приложения → Установленные приложения», оттуда же
удаляется.

**Тихая установка** (для администраторов и скриптов):

```bat
MistFences-v1.0.0-x64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

Полезные ключи: `/DIR="D:\Programs\MistFences"` — папка, `/TASKS=desktopicon` — ярлык на
рабочем столе (пустой `/TASKS=` — без ярлыка и без загрузки WebView2), `/LANG=ru` или
`/LANG=en` — язык установщика, `/LOG=<файл>` — журнал установки.

### Портативная версия (ZIP)

1. Распакуйте `MistFences-v<версия>-x64.zip` целиком в постоянную папку, например
   `C:\Programs\MistFences` (не на рабочий стол и не во временную папку). Файлы должны лежать
   вместе: `mistfences.exe`, `mistfences-watchdog.exe`, `mistfences-cli.exe`,
   `WebView2Loader.dll`.
2. Запустите `mistfences.exe`.
3. Чтобы настройки хранились в папке `config` рядом с программой (флешка, несколько копий),
   запускайте `mistfences.exe --portable` — например, через ярлык с этим ключом.
4. `restore-desktop.cmd` в той же папке — аварийный выход (см. [раздел 14](#14-если-что-то-пошло-не-так)).

Позже ZIP-копию можно «превратить» в установленную: укажите установщику ту же папку —
он заменит программы и не тронет остальные файлы (в том числе `config`).

## 3. Первый запуск

- MistFences **скрывает обычные значки Windows** и рисует значки рабочего стола сам: на той же
  сетке и с теми же подписями, что и Проводник. Файлы никуда не перемещаются.
- Появляется несколько областей по умолчанию — их можно переименовать, передвинуть или
  удалить; значки из удалённой области остаются на рабочем столе на том же месте.
- Включается **запуск вместе с Windows** (выключается в настройках, раздел «Общие →
  Рабочий стол и запуск»).
- В области уведомлений появляется значок MistFences. В Windows 11 новые значки прячутся под
  стрелкой «^» — его можно перетащить на панель задач. Если запустить MistFences ещё раз
  из меню «Пуск», откроется окно настроек уже работающей копии.

**Меню значка в области уведомлений** (правая кнопка):

- Показать все области / Скрыть все области
- Создать область, Показать папку на рабочем столе
- Показать области поверх окон (Ctrl+Alt+Space)
- Применить правила сейчас
- Показать значки рабочего стола / Снова скрыть значки рабочего стола
- Вернуть пропавшие значки рабочего стола — если какой-то значок перестал отображаться
- Сообщить о проблеме на GitHub
- Настройки…
- Выйти из MistFences — обычные значки Windows возвращаются

## 4. Области

| Действие | Как |
|---|---|
| Создать область | растяните рамку **правой** кнопкой мыши по пустому месту рабочего стола, не задевая значков, → «Создать область здесь» и сразу введите название. Или «Создать область» в меню трея. (Рамка правой кнопкой, задевшая значки, выделяет их и открывает их меню, как в Проводнике.) |
| Переместить | перетащите за заголовок; при перемещении области прилипают друг к другу и к краям экрана (Esc или правая кнопка — отменить) |
| Изменить размер | потяните за край или угол |
| Переименовать | «Переименовать…» в меню области |
| Свернуть / развернуть | двойной щелчок по заголовку или стрелка справа в заголовке. Свёрнутая область раскрывается при наведении |
| Меню области | правая кнопка по заголовку или пустому месту области |
| Удалить | «Удалить область» в меню; значки остаются на рабочем столе |
| Закрепить | «Закрепить положение и размер» — область нельзя случайно сдвинуть |
| Прикрепить к верху | «Прикрепить к верхнему краю» в параметрах области |
| Выделить несколько областей | рамка левой кнопкой по пустому месту рабочего стола выделяет задетые области (и значки); перетаскивание любой из выделенных областей двигает все |

**Параметры области** (меню области → «Параметры области…»): название, непрозрачность и
оттенок, цвет заголовка и вкладки, размер текста заголовка, размер и интервал значков,
автоматическая высота, «Скрывать, когда пусто», «Не скрывать двойным щелчком по рабочему
столу» и другое.

**Вид содержимого** (меню области → «Вид»): значки разных размеров, список или таблица
(с колонками «Имя», «Дата изменения», «Тип», «Размер»). **Сортировка**: вручную, по имени,
типу, дате изменения, размеру, числу открытий, с группировкой по дате.

## 5. Значки

Значки в областях и на рабочем столе ведут себя как в Проводнике:

- щелчок — выделить, Ctrl+щелчок — добавить к выделению, рамка левой кнопкой — выделить
  несколько, Ctrl+A — выделить все;
- двойной щелчок или Enter — открыть, правая кнопка — меню Проводника;
- F2 — переименовать (поле переименования Проводника), Del — в Корзину, Shift+Del — удалить;
- Ctrl+C / Ctrl+X / Ctrl+V — копировать, вырезать, вставить;
- Пробел — предпросмотр, если установлен [QuickLook](https://github.com/QL-Win/QuickLook);
- набор первых букв имени — переход к значку.

**Перетаскивание:**

- между областями и рабочим столом — меняется только место показа, файлы остаются на
  рабочем столе;
- на папку, ярлык папки или Корзину — файл **перемещается** туда через операцию копирования
  Windows (с её окнами подтверждения и ошибок). Ctrl — копировать, Alt — создать ярлык;
- в окно Проводника или другой программы — как обычное перетаскивание файлов.

Значки на рабочем столе выравниваются по сетке Проводника (отключается в «Общие → Рабочий
стол и запуск → Выравнивать значки рабочего стола по сетке»). Перед выравниванием всех
значков автоматически сохраняется снимок раскладки.

## 6. Вкладки

Несколько областей можно объединить в одну с вкладками:

- перетащите одну область за заголовок на заголовок другой или выберите «Объединить с…»;
- вкладки переключаются щелчком, колёсиком мыши над заголовком или Ctrl+Tab;
- перетащите вкладку в сторону, чтобы снова сделать её отдельной областью
  («Вынести в отдельную область»);
- «Расположение вкладок» (Настройки → Общие → Оформление): сверху или столбцом слева.

## 7. Папка на рабочем столе

«Показать папку на рабочем столе» (меню трея или области) создаёт область, которая
показывает содержимое любой папки, например «Загрузки» или папки проекта. Изменения в папке
появляются в области сразу. Файлы, брошенные в такую область, переносятся в эту папку
(Ctrl — копируются). Вложенные папки можно открывать прямо в области («Открывать вложенные
папки в этой области», Backspace — назад).

## 8. Правила сортировки

Правила решают, в какой области показывать новые элементы рабочего стола (файлы при этом
не перемещаются). Настройки → **Правила сортировки**:

- **Быстрое добавление** — готовые наборы «область + правило» одним щелчком (программы и
  ярлыки, документы, изображения, архивы и т. д.);
- **Создать правило** — условия: расширение, часть имени, тип (папка, программа, ярлык,
  обычный файл) и другие; действие — показать в выбранной области;
- правила проверяются сверху вниз, срабатывает первое подходящее — порядок меняется
  кнопками;
- «Автоматически распределять новые элементы рабочего стола» — правила работают на лету;
- «Элементы без подходящего правила» — вернуть в область «Рабочий стол» или оставить на месте;
- «Применить правила ко всем элементам рабочего стола» (или «Применить правила сейчас» в
  трее) — разложить всё заново.

## 9. Поверх окон и быстрое скрытие

- **Ctrl+Alt+Space** — показать все области поверх открытых окон («Поверх окон»). Можно
  сразу печатать — значки, подходящие под введённый текст, подсвечиваются. Esc — вернуть.
  Сочетание меняется в «Общие → Поверх окон».
- **Двойной щелчок по пустому месту рабочего стола** — спрятать или показать все области.
- **Win+D** — что делать с областями при показе рабочего стола, выбирается в «Общие →
  Рабочий стол и запуск».

## 10. Настройки

Открываются из меню трея («Настройки…») или повторным запуском MistFences. Разделы:

- **Общие**
  - *Оформление*: стиль (Fluent или Liquid Glass), своя тема (файлы в папке `themes`, см.
    [THEMES.md](https://github.com/Mist10X/MistFences/blob/main/docs/THEMES.md)), светлый или
    тёмный режим, цвет подписей, выравнивание заголовка, вкладки, размер и тонирование
    значков, стрелки ярлыков, язык интерфейса (русский, английский и ещё 8 языков).
  - *Рабочий стол и запуск*: запуск вместе с Windows, скрытие значков Windows, двойной щелчок
    по рабочему столу, выравнивание по сетке, «Создать область здесь», поведение при Win+D.
  - *Поведение*: раскрытие при наведении или по щелчку, сдвиг нижних областей, заголовок
    только при наведении, переключение вкладок при наведении, полосы прокрутки.
  - *Привязка и выравнивание*: прилипание, направляющие, расстояние между областями.
  - *Поверх окон*: включение, сочетание клавиш, затемнение фона.
- **Области** — параметры каждой области (то же, что «Параметры области…»).
- **Правила сортировки** — см. [раздел 8](#8-правила-сортировки).
- **Расположение и резервные копии** — снимки, мониторы, резервные копии, экспорт и импорт.
- **О программе** — версия, руководство, ссылка на GitHub, «Устранение неполадок».

MistFences ничего не отправляет в интернет: проверка обновлений, отправка отзывов и журналов
отключены. Сообщить о проблеме можно на странице
[Issues](https://github.com/Mist10X/MistFences/issues) («Сообщить о проблеме на GitHub» в
трее или в настройках).

## 11. Снимки, резервные копии, перенос на другой ПК

- **Снимки расположения** (Настройки → Расположение и резервные копии): «Сохранить текущее
  расположение», затем любой снимок можно восстановить. Перед рискованными действиями
  (выравнивание, импорт) снимок делается сам. Хранится до 20 снимков.
- **Несколько мониторов**: каждая область помнит свой монитор; при отключении монитора
  значки временно переезжают на основной и возвращаются, когда монитор снова подключён.
  «Поменять местами области на двух мониторах» — если Windows перепутала мониторы.
- **Резервные копии**: конфигурация копируется раз в день в папку `backups`.
- **Экспорт / импорт конфигурации** — один файл `.json` со всеми областями, правилами и
  настройками; его можно перенести на другой компьютер и импортировать там.

## 12. Переход со Stardock Fences

MistFences **не запускается, пока работает Stardock Fences**: две программы скрывали бы и
рисовали одни и те же значки. В этом случае при запуске появится сообщение с найденными
модулями Fences.

1. Закройте Fences (значок в трее → Exit) или удалите его через «Установленные приложения».
   Если Fences встроен в Проводник, после удаления перезапустите Проводник или выйдите из
   системы и войдите снова.
2. Запустите MistFences.

**Перенос раскладки из Fences 6 (необязательно).** MistFences умеет построить свою
конфигурацию по снимку Fences (`DailySnapshot*.xml` в `%APPDATA%\Stardock\Fences\Backups`).
Снимок только читается — Fences и файлы на рабочем столе не меняются. Делайте это, когда
MistFences закрыт. В PowerShell:

```powershell
$mf = "$env:LOCALAPPDATA\Programs\MistFences\mistfences.exe"   # или папка ZIP-версии
$snap = (Get-ChildItem "$env:APPDATA\Stardock\Fences\Backups\DailySnapshot*.xml" |
         Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
# 1) Пробный прогон: только отчёт, ничего не записывается
Start-Process $mf -Wait -ArgumentList '--import-fences', "`"$snap`"", '--config-dir', "`"$env:APPDATA\MistFences`"", '--dry-run'
notepad "$env:APPDATA\MistFences\import\preview-latest.md"
# 2) Импорт; --replace откладывает имеющийся config.json в сторону (config.before-import-*.json)
Start-Process $mf -Wait -ArgumentList '--import-fences', "`"$snap`"", '--config-dir', "`"$env:APPDATA\MistFences`"", '--replace'
notepad "$env:APPDATA\MistFences\import\result-latest.md"
```

Для портативной копии вместо `$env:APPDATA\MistFences` укажите её папку `config`. Если в
отчёте есть проблемы (например, монитор из снимка сейчас не подключён), импорт не
выполняется — отчёт объясняет, что именно мешает.

## 13. Командная строка

`mistfences-cli.exe` (в папке программы) управляет работающей копией MistFences из
терминала, скриптов и ИИ-агентов:

```powershell
cd "$env:LOCALAPPDATA\Programs\MistFences"
.\mistfences-cli.exe status            # состояние, версия, пути
.\mistfences-cli.exe fence list        # области
.\mistfences-cli.exe rule list         # правила
.\mistfences-cli.exe snapshot save     # снимок расположения
.\mistfences-cli.exe settings get      # все настройки
.\mistfences-cli.exe quit              # закрыть MistFences, как «Выйти» в трее
.\mistfences-cli.exe describe          # полный каталог команд
```

Подробности — в [CLI.md](https://github.com/Mist10X/MistFences/blob/main/docs/CLI.md)
(документ написан для исходного проекта: вместо `pecofence-cli` используйте
`mistfences-cli`).

## 14. Если что-то пошло не так

**Пропали значки рабочего стола.**

1. Меню трея → «Показать значки рабочего стола» или «Вернуть пропавшие значки рабочего стола».
2. Если MistFences не отвечает или его значка нет: «Пуск» → MistFences → **«Восстановить
   рабочий стол Windows»** (в ZIP-версии — `restore-desktop.cmd`). Это закрывает MistFences,
   возвращает значки Windows и сообщает, что сделано. Чтобы заодно выключить автозапуск:

   ```bat
   "%LOCALAPPDATA%\Programs\MistFences\mistfences-watchdog.exe" --restore --no-autostart
   ```
3. В крайнем случае: правая кнопка по рабочему столу → «Вид» → «Отображать значки
   рабочего стола».

**Защита от сбоев.** Рядом с MistFences работает сторожевой процесс: если программа
аварийно завершится, он сам вернёт значки Windows. Если MistFences падает три раза подряд в
течение 10 минут, следующий запуск будет «безопасным»: области показываются, но значки
Windows не скрываются.

**MistFences не запускается и пишет про Stardock Fences** — см. [раздел 12](#12-переход-со-stardock-fences).

**Не открываются настройки** — установите
[Microsoft Edge WebView2 Runtime](https://go.microsoft.com/fwlink/p/?LinkId=2124703).

**Журналы** для сообщения об ошибке: `%LOCALAPPDATA%\MistFences\mistfences.log` (текущий
запуск) и `mistfences.prev.log` (предыдущий). Приложите их к сообщению на
[GitHub Issues](https://github.com/Mist10X/MistFences/issues) — MistFences сам ничего не
отправляет.

## 15. Обновление и удаление

**Обновление.** Запустите установщик новой версии: он найдёт прежнюю папку, сам корректно
закроет работающий MistFences (значки Windows на это время вернутся), заменит программы и
предложит запустить новую версию. Области, правила и настройки сохраняются. Установить более
старую версию поверх новой нельзя — сначала удалите новую. Для ZIP-версии: выйдите из
MistFences и распакуйте новый архив в ту же папку поверх старого.

**Удаление.** «Параметры → Приложения → Установленные приложения → MistFences → Удалить».
Удаление закрывает MistFences, возвращает значки Windows, убирает ярлыки и запись
автозапуска этой копии и спрашивает, удалить ли также настройки, области и журналы
(по умолчанию — сохранить). Тихое удаление с удалением данных:

```bat
"%LOCALAPPDATA%\Programs\MistFences\unins000.exe" /VERYSILENT /REMOVEDATA=1
```

Файлы на рабочем столе не затрагиваются никогда.

**ZIP-версия**: выключите автозапуск в настройках, выйдите из MistFences через трей, удалите
папку программы и, если настройки не нужны, `%APPDATA%\MistFences` и
`%LOCALAPPDATA%\MistFences`.

## 16. Где хранятся данные

| Что | Где |
|---|---|
| Программа (установщик) | `%LOCALAPPDATA%\Programs\MistFences` |
| Настройки, области, правила, снимки (`config.json`, `config.bak`) | `%APPDATA%\MistFences` (с `--portable` — папка `config` рядом с программой) |
| Ежедневные резервные копии | `%APPDATA%\MistFences\backups` |
| Свои темы | `%APPDATA%\MistFences\themes` |
| Журналы, дампы сбоев, данные окна настроек | `%LOCALAPPDATA%\MistFences` |
| Автозапуск | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, значение `MistFences` |

MistFences — изменённая версия [PecoFence](https://github.com/DayuanJiang/PecoFence)
(Apache License 2.0). Лицензии — в файлах `LICENSE`, `NOTICE` и `THIRD-PARTY-LICENSES.txt`
рядом с программой. MistFences не связан со Stardock; Fences — товарный знак Stardock
Corporation.

---

## English

MistFences groups your desktop icons into **fences**, rolls them up into their title, sorts
new desktop items by rules and draws the remaining icons on the desktop itself, the way
Windows does. Files never move: fences only show what is on the desktop.

### Requirements and install

Windows 11 or Windows 10 22H2 (x64). The Settings window needs Microsoft Edge WebView2
Runtime (built into Windows 11; Setup offers to download it on Windows 10). No administrator
rights are needed. The program is not code-signed: if SmartScreen warns, choose
**More info → Run anyway**. Close Stardock Fences first — MistFences does not start next to
it.

- **Setup (recommended):** run `MistFences-v<version>-x64-setup.exe` from
  [Releases](https://github.com/Mist10X/MistFences/releases). It installs to
  `%LOCALAPPDATA%\Programs\MistFences` and adds a Start menu folder with *MistFences*,
  *Restore the Windows desktop* (the emergency way back) and *MistFences user guide*.
  Silent: `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`, plus `/DIR=…`, `/TASKS=desktopicon`,
  `/LANG=en`.
- **ZIP:** extract `MistFences-v<version>-x64.zip` to a permanent folder and run
  `mistfences.exe` (`--portable` keeps settings in a `config` folder beside it).
  `restore-desktop.cmd` is the emergency way back.

### Using it

On first start MistFences hides Windows' desktop icons and draws them itself on Explorer's
grid, creates a few fences and turns on start with Windows. Its notification-area icon
opens Settings and Exit (Exit gives Windows its icons back). Starting MistFences again opens
the running copy's Settings.

- **New fence:** drag a rectangle with the **right** mouse button over empty desktop space
  (touching no icon) → *Create a fence here*, then type its name.
- **Move / resize / rename:** drag the title, drag an edge, *Rename…* in the fence menu.
- **Roll up:** double-click the title; a rolled fence opens on hover.
- **Tabs:** drop a fence on another fence's title to merge them; drag a tab out to split.
- **Folder on the desktop:** a fence that shows any folder (*Show a folder on the desktop*).
- **Icons:** Explorer's keys and menus — Enter, F2, Del, Shift+Del, Ctrl+C / X / V / A, Space
  with QuickLook. Dropping on a folder, a folder shortcut or the Recycle Bin moves the file
  there (Ctrl copies, Alt makes a shortcut); moving icons between fences moves no file.
- **Sorting rules** (Settings → Sorting rules): show new desktop items in a fence by
  extension, name or type; first match wins.
- **Ctrl+Alt+Space** brings all fences above windows (type to search); double-clicking empty
  desktop hides or shows them all.
- **Layout snapshots and backups:** Settings → Layout & backups; export/import moves the whole
  configuration to another PC.
- **Command line:** `mistfences-cli.exe describe` lists every command (`status`, `fence list`,
  `rule list`, `snapshot save`, `quit`, …).
- **Import from Fences 6:** `mistfences.exe --import-fences <DailySnapshotN.xml> --config-dir
  "%APPDATA%\MistFences" [--dry-run] [--replace]` with MistFences closed; reports go to
  `import\preview-latest.md` / `result-latest.md` in that folder.

### Troubleshooting

- Icons gone: tray menu → *Show desktop icons*; if MistFences hangs, Start → MistFences →
  *Restore the Windows desktop* (ZIP: `restore-desktop.cmd`; `mistfences-watchdog.exe
  --restore --no-autostart` also turns autostart off); last resort: right-click the desktop →
  View → Show desktop icons.
- A watchdog restores the icons if MistFences crashes; after three crashes within ten
  minutes the next start leaves Windows' icons visible.
- Logs: `%LOCALAPPDATA%\MistFences\mistfences.log`. MistFences sends nothing over the
  network; report problems on [GitHub Issues](https://github.com/Mist10X/MistFences/issues).

### Update and uninstall

Run the newer setup: it closes MistFences properly, keeps your fences and settings and
starts the new version. Uninstall from Settings → Apps → Installed apps; it closes
MistFences, restores Windows' icons, removes this copy's startup entry and asks whether to
delete your settings too (silent: `unins000.exe /VERYSILENT /REMOVEDATA=1`). Desktop files are
never touched. Settings live in `%APPDATA%\MistFences`, logs in `%LOCALAPPDATA%\MistFences`.

MistFences is a modified version of [PecoFence](https://github.com/DayuanJiang/PecoFence)
(Apache License 2.0); see `LICENSE`, `NOTICE` and `THIRD-PARTY-LICENSES.txt`. Not affiliated
with Stardock; Fences is a trademark of Stardock Corporation.
