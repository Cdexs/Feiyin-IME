; Voice IME Inno Setup Script
; Version: 0.9.8
; Compile with: Inno Setup 6.x (https://jrsoftware.org/isinfo.php)
; RELEASE-ISS-FROM-PUBLISH-400：全部产物改从 ..\Publish\ 取；安装包不带模型（Gavin 定）。

#define MyAppName      "飞音语音输入"
#define MyAppNameEn    "Voice IME"
#define MyAppVersion   "0.9.8"
#define MyAppPublisher "Feiyin Voice Input Project"
#define MyAppURL       ""
#define MyAppExeName   "feiyin-ime.exe"
#define MyAppID        "{{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}"

[Setup]
; Unique application GUID
AppId={#MyAppID}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}

; Installation directory
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes

; Output
OutputDir=..\dist
OutputBaseFilename=VoiceIME-Setup-{#MyAppVersion}
#ifexist "..\assets\icons\app.ico"
SetupIconFile=..\assets\icons\app.ico
#endif

; Compression
Compression=lzma2/ultra64
SolidCompression=yes
CompressionThreads=auto

; UI
WizardStyle=modern
#ifexist "..\assets\icons\wizard_small.bmp"
WizardSmallImageFile=..\assets\icons\wizard_small.bmp
#endif

; Privileges
PrivilegesRequiredOverridesAllowed=dialog
PrivilegesRequired=lowest

; Architecture
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Uninstall
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName} {#MyAppVersion}

; Windows version requirement (Windows 10/11; DEC-000 — Win7 support removed 2026-04-17)
MinVersion=10.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "languages\ChineseSimplified.isl"

[Tasks]
Name: "autostart"; Description: "{cm:AutoStartProgram,{#MyAppName}}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; ============================================================================
; 全部程序产物取自 ..\Publish\（RELEASE-ISS-FROM-PUBLISH-400，显式白名单，不用通配）。
; Publish\voice-ime.iss 是本文件的副本，同在 Publish 目录内 ⇒ 相对路径 ..\Publish\ 解析到自身目录，两份逐字相同。
; ============================================================================

; ---- 主程序 / 设置 UI / 崩溃报告 ----
Source: "..\Publish\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\feiyin-ime-ui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\crash-reporter.exe"; DestDir: "{app}"; Flags: ignoreversion

; FLASH-P3-A（DEC-114）：输入法不再带任何本地模型 ⇒ ASR 运行库（sherpa-onnx / onnxruntime）、
; 翻译运行库（CTranslate2）、精解引擎运行库（llama.cpp / ggml / mtmd / libomp）与识别服务网关
; feiyin-asr-gateway.exe 全部删除（识别服务已是独立应用 flashasr，有自己的安装包）。

; ---- VC++ 2015-2022 x64 运行库，app-local（DEC-085 / RELEASE-VCRT-APPLOCAL-399；dumpbin 最小集）----
; FLASH-P3-A：debug 构建 dumpbin /dependents 显示三个 exe 只导入 VCRUNTIME140.dll；msvcp140 / msvcp140_1（C++ 模型库用）
; 与 vcomp140（CT2 OpenMP）大概率已不需要，待 release 包 dumpbin 复核后再删，本次保留。
Source: "..\Publish\msvcp140.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\msvcp140_1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\vcruntime140.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\vcruntime140_1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\vcomp140.dll"; DestDir: "{app}"; Flags: ignoreversion

; ---- 四张外置规则表（程序从 exe 同级读取，DEC-011；行号见 result.md）----
Source: "..\Publish\itn-rules.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\scene-rules.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\homophone-rules.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Publish\wordbook-rules.toml"; DestDir: "{app}"; Flags: ignoreversion

; ---- 非 Publish 产物（保留原样）----
Source: "..\assets\icons\*"; DestDir: "{app}\icons"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "..\assets\default-config.toml"; DestDir: "{app}"; Flags: ignoreversion

; ============================================================================
; 显式排除项（RELEASE-ISS-FROM-PUBLISH-400；原因见下）：
;   models\               : Gavin 决定「安装包不带模型」⇒ 用户首次使用时下载（当前程序无自动下载器，缺口见 result.md）
;   config.toml           : 用户运行时配置（安装器在 ssPostInstall 复制 default-config.toml 到 %APPDATA%，不得覆盖）
;   wordbook.sqlite       : 用户词库（运行时数据）
;   debug.log             : 运行时日志
;   version_check.json    : 程序自写的运行时产物
;   run-debug.bat         : 开发用脚本
;   voice-ime.iss         : 安装脚本自身（开发用）
;   cudnn64_9.dll         : dumpbin /dependents 显示无任何 exe/dll 导入（GPU 预留）⇒ 不带
;   libiomp5md.dll        : dumpbin /dependents 显示无任何 exe/dll 导入（INTEL OpenMP 预留；现用 VCOMP140）⇒ 不带
; ============================================================================

[Icons]
; Start Menu
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{#MyAppName} Settings"; Filename: "{app}\{#MyAppExeName}"; Parameters: "--settings"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"

; Desktop shortcut (optional, user can choose)
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: ""

[Registry]
; Auto-start with Windows (only if user selected the task)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "{#MyAppName}"; \
    ValueData: """{app}\{#MyAppExeName}"""; \
    Flags: uninsdeletevalue; Tasks: autostart

[Run]
; Launch after install
Filename: "{app}\{#MyAppExeName}"; Parameters: "--settings"; \
    Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; \
    Flags: nowait postinstall skipifsilent
; FLASH-P3-A：识别服务网关随本机识别服务删除 ⇒ 不再添加防火墙规则；只删旧版本留下的同名规则（升级清理）。
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Feiyin ASR Gateway"""; \
    Flags: runhidden; Check: IsAdminInstallMode

[UninstallRun]
; Kill running processes before uninstall (main / settings UI / crash reporter)
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM {#MyAppExeName}"; \
    Flags: runhidden; RunOnceId: "KillVoiceIME"
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM feiyin-ime-ui.exe"; \
    Flags: runhidden; RunOnceId: "KillVoiceIMEUI"
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM crash-reporter.exe"; \
    Flags: runhidden; RunOnceId: "KillCrashReporter"
; FLASH-P3-A：网关已删，只保留旧版本防火墙规则的清理。
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Feiyin ASR Gateway"""; \
    Flags: runhidden; RunOnceId: "DelAsrGatewayFirewall"; Check: IsAdminInstallMode

[UninstallDelete]
; Remove user data (optional, commented out by default to preserve user settings)
; Type: filesandordirs; Name: "{userappdata}\voice-ime"

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
var
  ConfigDir: String;
  ConfigFile: String;
  DefaultConfig: String;
begin
  if CurStep = ssPostInstall then
  begin
    // Copy default config to %APPDATA%\voice-ime\config.toml on first install
    ConfigDir := ExpandConstant('{userappdata}\voice-ime');
    ConfigFile := ConfigDir + '\config.toml';
    DefaultConfig := ExpandConstant('{app}\default-config.toml');

    if not DirExists(ConfigDir) then
      CreateDir(ConfigDir);

    // Only copy if user doesn't already have a config (preserve existing settings on upgrade)
    if not FileExists(ConfigFile) then
      FileCopy(DefaultConfig, ConfigFile, False);
  end;
end;
