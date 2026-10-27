# Безопасный предпросмотр MistFences

Предпросмотр показывает три тестовые группы с фиктивными значками прямо на рабочем столе,
рядом с работающим Fences 6, и ничего не меняет в Windows, Fences и ваших файлах.

## Что нужно один раз

* Rust (установлен в `%USERPROFILE%\.cargo`).
* Visual Studio Build Tools с C++ (у вас уже есть в `D:\devtools\BuildTools`).

## Запуск

Откройте PowerShell и выполните:

```powershell
cd D:\Dev\MistFences
./scripts/mistfences/build.ps1          # сборка (1–2 минуты в первый раз)
./scripts/mistfences/run-preview.ps1    # запуск предпросмотра
```

Если Windows не даёт запускать скрипты, разрешите их только для этого окна:
`Set-ExecutionPolicy -Scope Process Bypass`.

Группы появятся посередине основного монитора свёрнутыми в полоски — сверните окна
(или посмотрите на свободную часть рабочего стола), чтобы их увидеть. Наведите курсор на
полоску — группа раскроется; уведите — свернётся. Группы можно перетаскивать за заголовок
и растягивать за края.

## Закрытие

* Значок MistFences в трее ▸ «Выход», или
* `./scripts/mistfences/stop-preview.ps1`, или
* запуск с автозакрытием: `./scripts/mistfences/run-preview.ps1 -ExitAfter 60`.

## Где что лежит

| Что | Где |
|---|---|
| Исходники | `D:\Dev\MistFences` (Git, ветка `mistfences`) |
| Программа после сборки | `D:\Dev\MistFences\target\debug\mistfences.exe` (release: `target\release`) |
| Тестовые значки (песочница) | `D:\Dev\MistFences\.preview\Desktop` |
| Конфигурация предпросмотра | `D:\Dev\MistFences\.preview\config\config.json` |
| Журнал предпросмотра | `%LOCALAPPDATA%\MistFences\mistfences.preview.log` |
| Результаты замеров | `D:\Dev\MistFences\.preview\measurements` |

`./scripts/mistfences/run-preview.ps1 -Reset` начинает с исходной раскладки (тестовые
файлы сохраняются).

## Что предпросмотр не делает

* не скрывает значки рабочего стола Windows и не трогает Fences 6;
* не читает и не меняет настоящие папки рабочего стола;
* не пишет автозапуск, не регистрирует глобальные сочетания клавиш, не следит за мышью
  на рабочем столе;
* не открывает контекстное меню Проводника (в меню — только команды MistFences);
* не принимает перетаскивание извне и не отдаёт файлы в другие программы;
* меняет файлы только в `.preview\Desktop`.

## Проверки

```powershell
./scripts/mistfences/snapshot-system.ps1 -Out before.json   # до запуска
./scripts/mistfences/run-preview.ps1
./scripts/mistfences/probe-safety.ps1                         # попытки запрещённых действий
./scripts/mistfences/measure-preview.ps1                      # память и CPU
./scripts/mistfences/stop-preview.ps1
./scripts/mistfences/snapshot-system.ps1 -Out after.json
./scripts/mistfences/compare-snapshots.ps1 -Before before.json -After after.json
```
