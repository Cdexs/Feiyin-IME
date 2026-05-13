; Voice IME Inno Setup Script
; Version: 0.3.4
; Compile with: Inno Setup 6.x (https://jrsoftware.org/isinfo.php)

#define MyAppName      "飞音语音输入"
#define MyAppNameEn    "Voice IME"
#define MyAppVersion   "0.3.4"
#define MyAppPublisher "Feiyin Voice Input Project"
#define MyAppURL       ""
#define MyAppExeName   "voice-ime.exe"
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

; Windows version requirement (Windows 7+ for DEC-000 compatibility)
MinVersion=6.1

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "autostart"; Description: "{cm:AutoStartProgram,{#MyAppName}}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; Main executable
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; Runtime DLLs (required for ASR)
Source: "..\target\release\sherpa-onnx-c-api.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\sherpa-onnx-cxx-api.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\onnxruntime.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\onnxruntime_providers_shared.dll"; DestDir: "{app}"; Flags: ignoreversion

; ASR model (Paraformer-zh)
Source: "..\models\paraformer-zh-int8-2025-10-07\*"; DestDir: "{app}\models\paraformer-zh-int8-2025-10-07"; Flags: ignoreversion recursesubdirs createallsubdirs

; Application icon (if exists)
Source: "..\assets\icons\*"; DestDir: "{app}\icons"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist

; Default config template (shipped with app, copied to AppData on first install)
Source: "..\assets\default-config.toml"; DestDir: "{app}"; Flags: ignoreversion

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

[UninstallRun]
; Kill the running process before uninstall
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM {#MyAppExeName}"; \
    Flags: runhidden; RunOnceId: "KillVoiceIME"

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
