# MistFences

Бесплатная замена Stardock Fences для Windows 11 и Windows 10.
A free Stardock Fences alternative for Windows 11 and Windows 10. English below.

## Установка

1. Нужна Windows 11 или Windows 10 22H2 (64-бит) и Microsoft Edge WebView2 Runtime
   (в Windows 11 он уже есть).
2. Распакуйте архив целиком в постоянную папку, например `C:\Programs\MistFences`.
   Файлы должны лежать вместе: `mistfences.exe`, `mistfences-watchdog.exe`,
   `mistfences-cli.exe`, `WebView2Loader.dll`.
3. Запустите `mistfences.exe`. Программа не подписана, поэтому Windows SmartScreen может
   предупредить: нажмите «Подробнее» → «Выполнить в любом случае».
4. Если у вас работает Stardock Fences, закройте или удалите его: вместе они мешают друг другу.

## Что произойдёт при первом запуске

- MistFences скрывает обычные значки Windows и показывает значки рабочего стола сам: на
  той же сетке, с теми же подписями. Файлы никуда не перемещаются.
- Появляются области (группы значков), их можно переименовать, сдвинуть или удалить.
- Включается автозапуск при входе в Windows (выключается в настройках).
- Значок в области уведомлений: правая кнопка — настройки и выход.

## Как пользоваться

- **Новая область:** растяните рамку **правой** кнопкой мыши по пустому месту рабочего стола и
  выберите «Создать область здесь». Или пункт «Создать область» в меню значка в трее.
- **Сворачивание:** двойной щелчок по заголовку области сворачивает её; свёрнутая область
  раскрывается при наведении.
- **Значки:** перетаскивайте между областями и рабочим столом. Отпустите на папку, ярлык папки
  или Корзину — файл переместится туда (Ctrl — скопируется).
- **Как в Проводнике:** Del, Shift+Del, F2, Enter, Ctrl+C / Ctrl+X / Ctrl+V, Ctrl+A,
  меню правой кнопки, рамка левой кнопкой для выделения.
- **Ctrl+Alt+Space** — показать все области поверх окон.
- Перемещение, переименование, копирование и удаление работают с настоящими файлами, как в
  Проводнике. Правила автосортировки меняют только то, в какой области виден значок.

## Настройки и перенос

- Настройки хранятся в `%APPDATA%\MistFences`. Запуск с ключом `--portable` хранит их в
  папке `config` рядом с программой (удобно для флешки).
- Снимки раскладки: Настройки › Снимки. Перед выравниванием значков по сетке снимок
  делается автоматически.

## Удаление

1. В настройках выключите автозапуск.
2. Правая кнопка по значку в трее → «Выход». Обычные значки Windows вернутся.
3. Удалите папку программы и, если не нужны настройки, `%APPDATA%\MistFences`.

Если значки Windows не вернулись: пункт «Показать значки рабочего стола» в меню
трея, или правая кнопка по рабочему столу → «Вид» → «Показывать значки рабочего стола».

---

## Install (English)

1. Windows 11 or Windows 10 22H2 (x64) with Microsoft Edge WebView2 Runtime (built into
   Windows 11).
2. Extract the whole archive to a permanent folder and keep the files together.
3. Run `mistfences.exe`. The program is not code-signed: if SmartScreen warns, choose
   "More info" → "Run anyway".
4. Close or uninstall Stardock Fences if it runs; the two get in each other's way.

On first start MistFences hides Windows' own desktop icons and draws them itself on
Explorer's grid (no file is moved), creates fences you can rename, move or delete, and
turns on start with Windows (Settings). The tray icon opens Settings and Exit.

- New fence: draw a rectangle with the **right** mouse button on empty desktop space and
  choose "Create a fence here".
- Double-click a fence title to roll it up; it opens again on hover.
- Drag icons between fences and the desktop; drop on a folder, folder shortcut or the
  Recycle Bin to move the file there (Ctrl copies).
- Explorer keys work on icons: Del, Shift+Del, F2, Enter, Ctrl+C / X / V, Ctrl+A.
- Ctrl+Alt+Space brings all fences above windows.
- Settings live in `%APPDATA%\MistFences`; `--portable` keeps them in a `config` folder
  beside the program.

Uninstall: turn off start with Windows in Settings, choose Exit in the tray menu (Windows'
icons come back), then delete the folder and `%APPDATA%\MistFences`.

MistFences is a modified version of [PecoFence](https://github.com/DayuanJiang/PecoFence)
(Apache License 2.0). See `LICENSE`, `NOTICE` and `THIRD-PARTY-LICENSES.txt`.
