; MistFences installer. Compile through scripts/mistfences/package.ps1, which stages the same
; files as the ZIP and passes them in as PayloadDir.
#ifndef PayloadDir
  #error PayloadDir is required
#endif
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef Repository
  #error Repository is required
#endif
#define NumericVersion Copy(AppVersion, 1, Pos('-', AppVersion + '-') - 1)
#define ProductId "Mist10X.MistFences"
#define ProductName "MistFences"
; pecofence_core::brand: the Run value names, the data folders and the instance mutexes.
#define CurrentRunName "MistFences"
#define LegacyRunName "MistFences.Legacy"
#define DataDirName "MistFences"
#define AppMutexNames "Local\MistFences.SingleInstance,Local\MistFences.SingleInstance.Compat"
; Integration tests (scripts/test-windows-packaging.py) compile this same script with an
; isolated identity: its own uninstall entry, Run values, data folder name and mutex, so a test
; never touches a real MistFences. The packaging script never sets TestIdentity; test installers
; must never be shipped.
#ifdef TestIdentity
  #define ProductId "MistFences.InstallerTest." + TestIdentity
  #define ProductName "MistFences Installer Test " + TestIdentity
  #define CurrentRunName ProductId
  #define LegacyRunName ProductId + ".legacy"
  #define DataDirName ProductId
  #define AppMutexNames "Local\" + ProductId
#endif
#define WebView2Url "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
#define WebView2Client "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

[Setup]
AppId={#ProductId}
AppName={#ProductName}
AppVersion={#AppVersion}
AppVerName={#ProductName} {#AppVersion}
VersionInfoVersion={#NumericVersion}
VersionInfoProductName={#ProductName}
AppPublisher=MistFences contributors
AppPublisherURL=https://github.com/{#Repository}
AppSupportURL=https://github.com/{#Repository}/issues
AppUpdatesURL=https://github.com/{#Repository}/releases
DefaultDirName={localappdata}\Programs\{#ProductName}
DefaultGroupName={#ProductName}
; Per user, no administrator rights: the program, its Run value and its data are the user's.
PrivilegesRequired=lowest
UsePreviousAppDir=yes
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Windows 10 22H2 or Windows 11.
MinVersion=10.0.19045
SetupIconFile=..\..\crates\app\assets\pecofence.ico
UninstallDisplayIcon={app}\mistfences.exe
UninstallDisplayName={#ProductName}
OutputBaseFilename=MistFences-v{#AppVersion}-x64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
SetupMutex=Local\{#ProductId}.Setup
; No AppMutex: setup and uninstall ask a running MistFences to quit through its normal exit
; path (it shows the Windows desktop icons again on the way out), never by force.
CloseApplications=no
RestartApplications=no

[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "pt_BR"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "ko"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "zh_CN"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "zh_TW"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
; Offered only when the runtime is missing (Windows 10 without Edge's runtime): Settings needs it.
Name: "webview2"; Description: "{cm:WebView2Task}"; GroupDescription: "{cm:Requirements}"; Check: not WebView2Installed

[Files]
; Explicit payload: never collect config, caches or arbitrary files from a checkout.
Source: "{#PayloadDir}\mistfences-watchdog.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\mistfences-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\WebView2Loader.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\NOTICE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE-WebView2Loader.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\THIRD-PARTY-LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
; Last: an update aborted halfway (a file in use) keeps the old app, which still starts.
Source: "{#PayloadDir}\mistfences.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#ProductName}"; Filename: "{app}\mistfences.exe"; WorkingDir: "{app}"
; The emergency way back: quits MistFences and shows the Windows desktop icons.
Name: "{group}\{cm:RestoreDesktop}"; Filename: "{app}\mistfences-watchdog.exe"; Parameters: "--restore"; WorkingDir: "{app}"; Comment: "{cm:RestoreDesktopComment}"
Name: "{autodesktop}\{#ProductName}"; Filename: "{app}\mistfences.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[INI]
Filename: "{group}\{cm:UserGuide}.url"; Section: "InternetShortcut"; Key: "URL"; String: "https://github.com/{#Repository}/blob/main/docs/mistfences/USER-GUIDE.md"

[UninstallDelete]
Type: files; Name: "{group}\{cm:UserGuide}.url"

[Run]
Filename: "{app}\mistfences.exe"; Description: "{cm:LaunchProgram,{#ProductName}}"; Flags: nowait postinstall skipifsilent
; A silent upgrade that closed MistFences, or an update started by the app: start it again.
Filename: "{app}\mistfences.exe"; Flags: nowait; Check: RestartAfterUpdate

[CustomMessages]
ru.AlreadyInstalled=MistFences уже установлен в %1. Чтобы установить его в другую папку, сначала удалите его.
en.AlreadyInstalled=MistFences is already installed in %1. To install it in another folder, uninstall it first.
de.AlreadyInstalled=MistFences ist bereits in %1 installiert. Um es in einen anderen Ordner zu installieren, deinstalliere es zuerst.
fr.AlreadyInstalled=MistFences est déjà installé dans %1. Pour l'installer dans un autre dossier, désinstallez-le d'abord.
es.AlreadyInstalled=MistFences ya está instalado en %1. Para instalarlo en otra carpeta, desinstálalo primero.
pt_BR.AlreadyInstalled=O MistFences já está instalado em %1. Para instalá-lo em outra pasta, desinstale-o primeiro.
ja.AlreadyInstalled=MistFences はすでに %1 にインストールされています。別のフォルダーに入れるには、先にアンインストールしてください。
ko.AlreadyInstalled=MistFences가 이미 %1에 설치되어 있습니다. 다른 폴더에 설치하려면 먼저 제거하세요.
zh_CN.AlreadyInstalled=MistFences 已经装在 %1。想换个位置，请先卸载。
zh_TW.AlreadyInstalled=MistFences 已經安裝在 %1。想換個位置，請先解除安裝。
ru.InvalidDestination=В этой папке уже есть другие файлы. Выберите пустую папку или ту, где уже лежит MistFences.
en.InvalidDestination=This folder already contains other files. Choose an empty folder, or the folder that already contains MistFences.
de.InvalidDestination=Dieser Ordner enthält bereits andere Dateien. Wähle einen leeren Ordner oder den Ordner, in dem MistFences schon liegt.
fr.InvalidDestination=Ce dossier contient déjà d'autres fichiers. Choisissez un dossier vide ou celui qui contient déjà MistFences.
es.InvalidDestination=Esta carpeta ya tiene otros archivos. Elige una carpeta vacía o la que ya contiene MistFences.
pt_BR.InvalidDestination=Esta pasta já tem outros arquivos. Escolha uma pasta vazia ou a que já contém o MistFences.
ja.InvalidDestination=このフォルダーには別のファイルがあります。空のフォルダーか、MistFences が入っているフォルダーを選んでください。
ko.InvalidDestination=이 폴더에는 이미 다른 파일이 있습니다. 빈 폴더나 MistFences가 들어 있는 폴더를 선택하세요.
zh_CN.InvalidDestination=这个文件夹里已经有别的文件了。请选一个空文件夹，或者原来放 MistFences 的文件夹。
zh_TW.InvalidDestination=這個資料夾裡已經有其他檔案。請選一個空資料夾，或原本放 MistFences 的資料夾。
ru.NewerVersion=Уже установлена более новая версия MistFences. Чтобы установить старую версию, сначала удалите новую. Настройки и области сохранятся.
en.NewerVersion=A newer version of MistFences is already installed. Uninstall it first to install this older version. Your settings and fences are kept.
de.NewerVersion=Eine neuere Version von MistFences ist bereits installiert. Um diese ältere Version zu installieren, deinstalliere sie zuerst. Deine Einstellungen und Bereiche bleiben erhalten.
fr.NewerVersion=Une version plus récente de MistFences est déjà installée. Pour installer cette version plus ancienne, désinstallez-la d'abord. Vos réglages et vos groupes sont conservés.
es.NewerVersion=Ya hay una versión más reciente de MistFences instalada. Para instalar esta versión anterior, desinstálala primero. Tus ajustes y grupos se conservan.
pt_BR.NewerVersion=Uma versão mais recente do MistFences já está instalada. Para instalar esta versão anterior, desinstale-a primeiro. Suas configurações e grupos são mantidos.
ja.NewerVersion=新しいバージョンの MistFences がインストールされています。この古いバージョンを入れるには、先にアンインストールしてください。設定とフェンスは残ります。
ko.NewerVersion=더 최신 버전의 MistFences가 설치되어 있습니다. 이 이전 버전을 설치하려면 먼저 제거하세요. 설정과 펜스는 그대로 남습니다.
zh_CN.NewerVersion=已经装了更新的 MistFences。要装这个旧版本，请先卸载。设置和栅栏都会保留。
zh_TW.NewerVersion=已經安裝了較新的 MistFences。要安裝這個舊版本，請先解除安裝。設定和圍欄都會保留。
ru.WillClose=MistFences запущен: перед установкой он будет закрыт (значки рабочего стола Windows на это время вернутся).
en.WillClose=MistFences is running: Setup closes it before installing (the Windows desktop icons show meanwhile).
de.WillClose=MistFences läuft: Setup beendet es vor der Installation (die Windows-Desktopsymbole erscheinen so lange wieder).
fr.WillClose=MistFences est en cours d'exécution : le programme d'installation le ferme avant d'installer (les icônes du Bureau Windows réapparaissent entre-temps).
es.WillClose=MistFences está en ejecución: el instalador lo cerrará antes de instalar (mientras tanto se muestran los iconos del escritorio de Windows).
pt_BR.WillClose=O MistFences está em execução: o instalador vai fechá-lo antes de instalar (os ícones da área de trabalho do Windows voltam enquanto isso).
ja.WillClose=MistFences が実行中です。インストールの前に終了します（その間は Windows のデスクトップ アイコンが表示されます）。
ko.WillClose=MistFences가 실행 중입니다. 설치 전에 종료합니다(그동안 Windows 바탕 화면 아이콘이 표시됩니다).
zh_CN.WillClose=MistFences 正在运行：安装前会先退出它（这期间会显示 Windows 桌面图标）。
zh_TW.WillClose=MistFences 正在執行：安裝前會先結束它（這段期間會顯示 Windows 桌面圖示）。
ru.CloseFailed=MistFences не закрылся. Закройте его из меню значка в области уведомлений (правая кнопка → «Выйти») и запустите установку ещё раз.
en.CloseFailed=MistFences did not close. Exit it from its notification area icon (right-click → Exit), then run Setup again.
de.CloseFailed=MistFences wurde nicht beendet. Beende es über sein Symbol im Infobereich (Rechtsklick → Beenden) und starte Setup erneut.
fr.CloseFailed=MistFences ne s'est pas fermé. Quittez-le depuis son icône de la zone de notification (clic droit → Quitter), puis relancez l'installation.
es.CloseFailed=MistFences no se cerró. Ciérralo desde su icono del área de notificación (clic derecho → Salir) y vuelve a ejecutar el instalador.
pt_BR.CloseFailed=O MistFences não fechou. Saia dele pelo ícone na área de notificação (botão direito → Sair) e execute o instalador novamente.
ja.CloseFailed=MistFences が終了しませんでした。通知領域のアイコンを右クリックして終了してから、もう一度セットアップを実行してください。
ko.CloseFailed=MistFences가 종료되지 않았습니다. 알림 영역 아이콘을 마우스 오른쪽 단추로 클릭해 종료한 다음 설치를 다시 실행하세요.
zh_CN.CloseFailed=MistFences 没有退出。请在通知区域图标上右键选择退出，然后重新运行安装程序。
zh_TW.CloseFailed=MistFences 沒有結束。請在通知區域圖示上按右鍵選擇結束，然後重新執行安裝程式。
ru.UninstallCloseFailed=MistFences не закрылся. Закройте его из меню значка в области уведомлений (правая кнопка → «Выйти») и запустите удаление ещё раз.
en.UninstallCloseFailed=MistFences did not close. Exit it from its notification area icon (right-click → Exit), then uninstall again.
de.UninstallCloseFailed=MistFences wurde nicht beendet. Beende es über sein Symbol im Infobereich (Rechtsklick → Beenden) und deinstalliere erneut.
fr.UninstallCloseFailed=MistFences ne s'est pas fermé. Quittez-le depuis son icône de la zone de notification (clic droit → Quitter), puis relancez la désinstallation.
es.UninstallCloseFailed=MistFences no se cerró. Ciérralo desde su icono del área de notificación (clic derecho → Salir) y vuelve a desinstalarlo.
pt_BR.UninstallCloseFailed=O MistFences não fechou. Saia dele pelo ícone na área de notificação (botão direito → Sair) e desinstale novamente.
ja.UninstallCloseFailed=MistFences が終了しませんでした。通知領域のアイコンを右クリックして終了してから、もう一度アンインストールしてください。
ko.UninstallCloseFailed=MistFences가 종료되지 않았습니다. 알림 영역 아이콘을 마우스 오른쪽 단추로 클릭해 종료한 다음 다시 제거하세요.
zh_CN.UninstallCloseFailed=MistFences 没有退出。请在通知区域图标上右键选择退出，然后重新卸载。
zh_TW.UninstallCloseFailed=MistFences 沒有結束。請在通知區域圖示上按右鍵選擇結束，然後重新解除安裝。
ru.RestoreDesktop=Восстановить рабочий стол Windows
en.RestoreDesktop=Restore the Windows desktop
de.RestoreDesktop=Windows-Desktop wiederherstellen
fr.RestoreDesktop=Restaurer le Bureau Windows
es.RestoreDesktop=Restaurar el escritorio de Windows
pt_BR.RestoreDesktop=Restaurar a área de trabalho do Windows
ja.RestoreDesktop=Windows のデスクトップを元に戻す
ko.RestoreDesktop=Windows 바탕 화면 복원
zh_CN.RestoreDesktop=恢复 Windows 桌面
zh_TW.RestoreDesktop=還原 Windows 桌面
ru.RestoreDesktopComment=Закрывает MistFences и снова показывает значки рабочего стола Windows
en.RestoreDesktopComment=Quits MistFences and shows the Windows desktop icons again
de.RestoreDesktopComment=Beendet MistFences und zeigt die Windows-Desktopsymbole wieder an
fr.RestoreDesktopComment=Quitte MistFences et réaffiche les icônes du Bureau Windows
es.RestoreDesktopComment=Cierra MistFences y vuelve a mostrar los iconos del escritorio de Windows
pt_BR.RestoreDesktopComment=Fecha o MistFences e mostra de novo os ícones da área de trabalho do Windows
ja.RestoreDesktopComment=MistFences を終了し、Windows のデスクトップ アイコンを再表示します
ko.RestoreDesktopComment=MistFences를 종료하고 Windows 바탕 화면 아이콘을 다시 표시합니다
zh_CN.RestoreDesktopComment=退出 MistFences 并重新显示 Windows 桌面图标
zh_TW.RestoreDesktopComment=結束 MistFences 並重新顯示 Windows 桌面圖示
ru.UserGuide=Руководство MistFences
en.UserGuide=MistFences user guide
de.UserGuide=MistFences-Anleitung
fr.UserGuide=Guide de MistFences
es.UserGuide=Guía de MistFences
pt_BR.UserGuide=Guia do MistFences
ja.UserGuide=MistFences ユーザー ガイド
ko.UserGuide=MistFences 사용 설명서
zh_CN.UserGuide=MistFences 使用手册
zh_TW.UserGuide=MistFences 使用手冊
ru.Requirements=Необходимые компоненты:
en.Requirements=Required components:
de.Requirements=Benötigte Komponenten:
fr.Requirements=Composants nécessaires :
es.Requirements=Componentes necesarios:
pt_BR.Requirements=Componentes necessários:
ja.Requirements=必要なコンポーネント:
ko.Requirements=필요한 구성 요소:
zh_CN.Requirements=必需组件：
zh_TW.Requirements=必要元件：
ru.WebView2Task=Скачать и установить Microsoft Edge WebView2 Runtime (нужен для окна настроек)
en.WebView2Task=Download and install Microsoft Edge WebView2 Runtime (needed for the Settings window)
de.WebView2Task=Microsoft Edge WebView2 Runtime herunterladen und installieren (für das Einstellungsfenster nötig)
fr.WebView2Task=Télécharger et installer Microsoft Edge WebView2 Runtime (nécessaire à la fenêtre des réglages)
es.WebView2Task=Descargar e instalar Microsoft Edge WebView2 Runtime (necesario para la ventana de ajustes)
pt_BR.WebView2Task=Baixar e instalar o Microsoft Edge WebView2 Runtime (necessário para a janela de configurações)
ja.WebView2Task=Microsoft Edge WebView2 ランタイムをダウンロードしてインストールする（設定画面に必要）
ko.WebView2Task=Microsoft Edge WebView2 런타임 다운로드 및 설치(설정 창에 필요)
zh_CN.WebView2Task=下载并安装 Microsoft Edge WebView2 运行时（设置窗口需要）
zh_TW.WebView2Task=下載並安裝 Microsoft Edge WebView2 執行階段（設定視窗需要）
ru.WebView2Installing=Установка Microsoft Edge WebView2 Runtime… Это может занять несколько минут.
en.WebView2Installing=Installing Microsoft Edge WebView2 Runtime… This can take a few minutes.
de.WebView2Installing=Microsoft Edge WebView2 Runtime wird installiert… Das kann einige Minuten dauern.
fr.WebView2Installing=Installation de Microsoft Edge WebView2 Runtime… Cela peut prendre quelques minutes.
es.WebView2Installing=Instalando Microsoft Edge WebView2 Runtime… Puede tardar unos minutos.
pt_BR.WebView2Installing=Instalando o Microsoft Edge WebView2 Runtime… Isso pode levar alguns minutos.
ja.WebView2Installing=Microsoft Edge WebView2 ランタイムをインストールしています… 数分かかることがあります。
ko.WebView2Installing=Microsoft Edge WebView2 런타임을 설치하는 중… 몇 분 정도 걸릴 수 있습니다.
zh_CN.WebView2Installing=正在安装 Microsoft Edge WebView2 运行时…可能需要几分钟。
zh_TW.WebView2Installing=正在安裝 Microsoft Edge WebView2 執行階段…可能需要幾分鐘。
ru.WebView2Failed=Не удалось установить Microsoft Edge WebView2 Runtime (%1). MistFences работает и без него, но окну настроек он нужен: скачайте его по ссылке %2 и запустите.
en.WebView2Failed=Microsoft Edge WebView2 Runtime could not be installed (%1). MistFences runs without it, but its Settings window needs it: download it from %2 and run it.
de.WebView2Failed=Microsoft Edge WebView2 Runtime konnte nicht installiert werden (%1). MistFences läuft auch ohne, aber das Einstellungsfenster braucht es: lade es von %2 herunter und führe es aus.
fr.WebView2Failed=Microsoft Edge WebView2 Runtime n'a pas pu être installé (%1). MistFences fonctionne sans, mais sa fenêtre des réglages en a besoin : téléchargez-le depuis %2 et lancez-le.
es.WebView2Failed=No se pudo instalar Microsoft Edge WebView2 Runtime (%1). MistFences funciona sin él, pero su ventana de ajustes lo necesita: descárgalo de %2 y ejecútalo.
pt_BR.WebView2Failed=Não foi possível instalar o Microsoft Edge WebView2 Runtime (%1). O MistFences funciona sem ele, mas a janela de configurações precisa dele: baixe-o em %2 e execute-o.
ja.WebView2Failed=Microsoft Edge WebView2 ランタイムをインストールできませんでした（%1）。MistFences はなくても動きますが、設定画面には必要です。%2 からダウンロードして実行してください。
ko.WebView2Failed=Microsoft Edge WebView2 런타임을 설치하지 못했습니다(%1). MistFences는 없어도 실행되지만 설정 창에는 필요합니다. %2에서 내려받아 실행하세요.
zh_CN.WebView2Failed=无法安装 Microsoft Edge WebView2 运行时（%1）。MistFences 没有它也能运行，但设置窗口需要它：请从 %2 下载并运行。
zh_TW.WebView2Failed=無法安裝 Microsoft Edge WebView2 執行階段（%1）。MistFences 沒有它也能執行，但設定視窗需要它：請從 %2 下載並執行。
ru.RemoveData=Удалить также настройки MistFences, области, снимки раскладки и журналы?%n%nВыберите «Нет», чтобы сохранить их для следующей установки. Файлы на рабочем столе не затрагиваются в любом случае.
en.RemoveData=Also delete your MistFences settings, fences, layout snapshots and logs?%n%nChoose No to keep them for a later installation. Files on your desktop are not touched either way.
de.RemoveData=Auch die Einstellungen, Bereiche, Layout-Schnappschüsse und Protokolle von MistFences löschen?%n%nWähle Nein, um sie für eine spätere Installation zu behalten. Dateien auf dem Desktop bleiben in jedem Fall unverändert.
fr.RemoveData=Supprimer aussi les réglages, groupes, instantanés de disposition et journaux de MistFences ?%n%nChoisissez Non pour les garder pour une installation ultérieure. Les fichiers du Bureau ne sont jamais touchés.
es.RemoveData=¿Eliminar también los ajustes, grupos, instantáneas de diseño y registros de MistFences?%n%nElige No para conservarlos para una instalación posterior. Los archivos del escritorio no se tocan en ningún caso.
pt_BR.RemoveData=Excluir também as configurações, grupos, instantâneos de layout e logs do MistFences?%n%nEscolha Não para mantê-los para uma instalação futura. Os arquivos da área de trabalho não são alterados de qualquer forma.
ja.RemoveData=MistFences の設定、フェンス、レイアウトのスナップショット、ログも削除しますか？%n%n後で再インストールするときのために残す場合は［いいえ］を選んでください。デスクトップ上のファイルはどちらの場合も変更されません。
ko.RemoveData=MistFences 설정, 펜스, 레이아웃 스냅숏과 로그도 삭제할까요?%n%n나중에 다시 설치할 때 쓰려면 [아니요]를 선택하세요. 바탕 화면의 파일은 어느 쪽이든 변경되지 않습니다.
zh_CN.RemoveData=同时删除 MistFences 的设置、栅栏、布局快照和日志吗？%n%n选择“否”可保留它们，供以后重新安装使用。无论怎么选，桌面上的文件都不会被改动。
zh_TW.RemoveData=同時刪除 MistFences 的設定、圍欄、版面配置快照和記錄檔嗎？%n%n選擇「否」可保留它們，供日後重新安裝使用。無論怎麼選，桌面上的檔案都不會被變更。

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#ProductId}_is1';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  WebView2ClientKey = 'Software\Microsoft\EdgeUpdate\Clients\{#WebView2Client}';

var
  Updating, Installed, WasRunning: Boolean;

function OpenProcess(Access: DWORD; Inherit: BOOL; Pid: DWORD): THandle;
  external 'OpenProcess@kernel32.dll stdcall';
function WaitForSingleObject(Handle: THandle; Milliseconds: DWORD): DWORD;
  external 'WaitForSingleObject@kernel32.dll stdcall';
function CloseHandle(Handle: THandle): BOOL;
  external 'CloseHandle@kernel32.dll stdcall';

function AppRunning: Boolean;
begin
  Result := CheckForMutexes('{#AppMutexNames}');
end;

{ The Evergreen runtime registers its version per machine (32-bit view) or per user. }
function WebView2Version(RootKey: Integer): String;
begin
  if not RegQueryStringValue(RootKey, WebView2ClientKey, 'pv', Result) then
    Result := '';
end;

function WebView2Installed: Boolean;
var
  Version: String;
begin
  Version := WebView2Version(HKLM32);
  if (Version = '') or (Version = '0.0.0.0') then
    Version := WebView2Version(HKCU);
  Result := (Version <> '') and (Version <> '0.0.0.0');
end;

{ /UPDATE=<pid>,<pid>...: started by an updater that exits right away. Wait, 30 s at most in
  all, until it, its watchdogs and its instance lock are gone, so no file is in use. Every pid
  is opened first, so one that exits early cannot be reused by another process meanwhile. }
function InitializeSetup: Boolean;
var
  Pids: String;
  Comma, Count, I, Round: Integer;
  Processes: array of THandle;
  Waiting: Boolean;
begin
  Pids := ExpandConstant('{param:UPDATE|}');
  Updating := Pids <> '';
  Count := 0;
  while Pids <> '' do begin
    Comma := Pos(',', Pids);
    if Comma = 0 then
      Comma := Length(Pids) + 1;
    SetArrayLength(Processes, Count + 1);
    Processes[Count] := OpenProcess($00100000 { SYNCHRONIZE }, False, StrToIntDef(Copy(Pids, 1, Comma - 1), 0));
    if Processes[Count] <> 0 then
      Count := Count + 1;
    Delete(Pids, 1, Comma);
  end;
  if Updating then begin
    for Round := 0 to 300 do begin
      Waiting := AppRunning;
      for I := 0 to Count - 1 do
        if WaitForSingleObject(Processes[I], 0) <> 0 then
          Waiting := True;
      if not Waiting or (Round = 300) then begin
        Log(Format('Update: waited %d ms for MistFences to exit', [Round * 100]));
        Break;
      end;
      Sleep(100);
    end;
  end;
  for I := 0 to Count - 1 do
    CloseHandle(Processes[I]);
  Result := True;
end;

{ Asks a running MistFences to quit as its tray menu's Exit does, through
  `mistfences-watchdog --restore --quiet` (it also shows the Windows desktop icons if they are
  still hidden), and waits for it. Never kills it. True when it is not running (any more). }
function CloseApp(const Watchdog: String): Boolean;
var
  Code, Round: Integer;
begin
  Result := not AppRunning;
  if Result then
    Exit;
#ifdef TestIdentity
  Log('Test identity: would ask MistFences to quit with ' + Watchdog);
#else
  Log('MistFences is running: asking it to quit');
  if not Exec(Watchdog, '--restore --quiet', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Log(Format('Could not run %s: %s', [Watchdog, SysErrorMessage(Code)]));
  for Round := 0 to 50 do begin
    if not AppRunning then begin
      Result := True;
      Break;
    end;
    Sleep(100);
  end;
  { Its crash watchdog leaves right after it; give it that moment before files are replaced. }
  if Result then
    Sleep(500);
#endif
  Log(Format('MistFences closed: %d', [Ord(Result)]));
end;

{ Test installers carry the real app: they log the start instead of running it. }
function StartApp(const Executable: String): Boolean;
#ifndef TestIdentity
var
  Code: Integer;
#endif
begin
#ifdef TestIdentity
  Log('Test identity: would start ' + Executable);
  Result := False;
#else
  Result := ExecAsOriginalUser(Executable, '', '', SW_SHOWNORMAL, ewNoWait, Code);
#endif
end;

function RestartAfterUpdate: Boolean;
begin
  { An interactive setup has the "Launch MistFences" box on its last page instead. }
  Result := Updating or (WasRunning and WizardSilent);
#ifdef TestIdentity
  if Result then
    StartApp(ExpandConstant('{app}\mistfences.exe'));
  Result := False;
#endif
end;

{ An update or upgrade that failed or was cancelled after MistFences was closed must not leave
  the user without it. }
procedure DeinitializeSetup;
var
  PreviousDir: String;
begin
  if (Updating or WasRunning) and not Installed and not AppRunning and
    RegQueryStringValue(HKCU, UninstallKey, 'InstallLocation', PreviousDir) and
    FileExists(AddBackslash(PreviousDir) + 'mistfences.exe') then
    StartApp(AddBackslash(PreviousDir) + 'mistfences.exe');
end;

function SameDirectory(const Left, Right: String): Boolean;
begin
  Result := CompareText(RemoveBackslashUnlessRoot(ExpandFileName(Left)),
    RemoveBackslashUnlessRoot(ExpandFileName(Right))) = 0;
end;

function DirectoryHasEntries(const Path: String): Boolean;
var
  Entry: TFindRec;
begin
  Result := False;
  if FindFirst(AddBackslash(Path) + '*', Entry) then begin
    try
      repeat
        if (Entry.Name <> '.') and (Entry.Name <> '..') then begin
          Result := True;
          Break;
        end;
      until not FindNext(Entry);
    finally
      FindClose(Entry);
    end;
  end;
end;

function UpdateReadyMemo(Space, NewLine, MemoUserInfoInfo, MemoDirInfo, MemoTypeInfo,
  MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := MemoDirInfo;
  if MemoGroupInfo <> '' then
    Result := Result + NewLine + NewLine + MemoGroupInfo;
  if MemoTasksInfo <> '' then
    Result := Result + NewLine + NewLine + MemoTasksInfo;
  if AppRunning then
    Result := Result + NewLine + NewLine + CustomMessage('WillClose');
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  PreviousDir, PreviousVersion: String;
  PreviousNumber, NewNumber: Int64;
begin
  Result := '';
  { A registration whose folder was deleted by hand no longer pins the location. }
  if RegQueryStringValue(HKCU, UninstallKey, 'InstallLocation', PreviousDir) and
    DirExists(PreviousDir) then begin
    if not SameDirectory(PreviousDir, ExpandConstant('{app}')) then begin
      Result := FmtMessage(CustomMessage('AlreadyInstalled'), [RemoveBackslashUnlessRoot(PreviousDir)]);
      Exit;
    end;
    if RegQueryStringValue(HKCU, UninstallKey, 'DisplayVersion', PreviousVersion) then begin
      { Compare the numeric release version; prerelease suffixes do not affect ordering. }
      if Pos('-', PreviousVersion) > 0 then
        PreviousVersion := Copy(PreviousVersion, 1, Pos('-', PreviousVersion) - 1);
      if StrToVersion(PreviousVersion, PreviousNumber) and
        StrToVersion('{#NumericVersion}', NewNumber) then
        if ComparePackedVersion(PreviousNumber, NewNumber) > 0 then begin
          Result := CustomMessage('NewerVersion');
          Exit;
        end;
    end;
  end else if DirectoryHasEntries(ExpandConstant('{app}')) and
    not FileExists(ExpandConstant('{app}\mistfences.exe')) then begin
    { A folder the ZIP was extracted into is upgraded in place; unrelated files are not. }
    Result := CustomMessage('InvalidDestination');
    Exit;
  end;
  { Last, so a refused install leaves a running MistFences alone. The new watchdog speaks the
    same pipe protocol as the installed one. }
  if AppRunning then begin
    WasRunning := True;
    ExtractTemporaryFile('mistfences-watchdog.exe');
    if not CloseApp(ExpandConstant('{tmp}\mistfences-watchdog.exe')) then
      Result := CustomMessage('CloseFailed');
  end;
end;

{ Downloads the Evergreen bootstrapper and runs it; it installs the runtime per user when it
  is not elevated. MistFences runs without the runtime, so a failure only warns. }
procedure InstallWebView2;
var
  Setup: String;
  Code: Integer;
  Error: String;
begin
#ifdef TestIdentity
  Log('Test identity: would install WebView2 Runtime');
#else
  Error := '';
  WizardForm.StatusLabel.Caption := CustomMessage('WebView2Installing');
  try
    DownloadTemporaryFile('{#WebView2Url}', 'MicrosoftEdgeWebview2Setup.exe', '', nil);
    Setup := ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe');
    if not Exec(Setup, '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, Code) then
      Error := SysErrorMessage(Code)
    else if (Code <> 0) and not WebView2Installed then
      Error := Format('0x%.8x', [Code]);
  except
    Error := GetExceptionMessage;
  end;
  if Error <> '' then begin
    Log('WebView2 Runtime not installed: ' + Error);
    if not WizardSilent then
      MsgBox(FmtMessage(CustomMessage('WebView2Failed'), [Error, '{#WebView2Url}']), mbError, MB_OK);
  end else
    Log('WebView2 Runtime installed');
#endif
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then begin
    Installed := True;
    { The files are in place; the runtime comes before the last page offers to start
      MistFences. }
    if WizardIsTaskSelected('webview2') and not WebView2Installed then
      InstallWebView2;
  end;
end;

{ The program of a Run command line: the quoted part, or the whole line. }
function CommandProgram(const Command: String): String;
var
  Rest: String;
begin
  Result := Trim(Command);
  if Copy(Result, 1, 1) = '"' then begin
    Rest := Copy(Result, 2, Length(Result) - 1);
    if Pos('"', Rest) > 0 then
      Result := Copy(Rest, 1, Pos('"', Rest) - 1)
    else
      Result := Rest;
  end;
end;

{ Removes a startup entry only when it starts this installation (with or without arguments
  such as --portable): an entry of another MistFences copy stays. }
procedure RemoveOwnedStartupValue(const Name: String);
var
  Command: String;
begin
  if RegQueryStringValue(HKCU, RunKey, Name, Command) then
    if CompareText(CommandProgram(Command), ExpandConstant('{app}\mistfences.exe')) = 0 then
      RegDeleteValue(HKCU, RunKey, Name);
end;

function InitializeUninstall: Boolean;
begin
  Result := CloseApp(ExpandConstant('{app}\mistfences-watchdog.exe'));
  if not Result and not UninstallSilent then
    MsgBox(CustomMessage('UninstallCloseFailed'), mbError, MB_OK);
end;

{ Settings, fences and logs live outside the program folder (%APPDATA% and %LOCALAPPDATA%), and
  a portable copy's in config\ next to it. They stay unless the user asks, or /REMOVEDATA=1. }
function RemoveDataWanted: Boolean;
begin
  if UninstallSilent then
    Result := ExpandConstant('{param:REMOVEDATA|0}') = '1'
  else
    Result := MsgBox(CustomMessage('RemoveData'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
end;

procedure RemoveData;
begin
  DelTree(ExpandConstant('{userappdata}\{#DataDirName}'), True, True, True);
  DelTree(ExpandConstant('{localappdata}\{#DataDirName}'), True, True, True);
  DelTree(ExpandConstant('{app}\config'), True, True, True);
  Log('MistFences data removed');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then begin
    RemoveOwnedStartupValue('{#CurrentRunName}');
    RemoveOwnedStartupValue('{#LegacyRunName}');
    if RemoveDataWanted then
      RemoveData;
  end;
  if CurUninstallStep = usDone then
    Log('MistFences uninstall cleanup completed');
end;
