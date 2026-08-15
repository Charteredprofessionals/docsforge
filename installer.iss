; DocForge Inno Setup installer script
; Builds an updated setup .exe from the current release binary.
; Compile with: "C:\Users\cscha\AppData\Local\Programs\Inno Setup 6\ISCC.exe" installer.iss

#define MyAppName "DocForge"
#define MyAppVersion "2.0.0"
#define MyAppPublisher "DocForge"
#define MyAppExeName "docforge.exe"

[Setup]
AppId={{E3C7A1B2-9F4D-4C8E-B1A2-7D5E6F3A9C10}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=exports\windows
OutputBaseFilename=DocForge_{#MyAppVersion}_x64-setup
SetupIconFile=src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64
ArchitecturesAllowed=x64
PrivilegesRequired=admin
VersionInfoVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Document Automation
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; The Tauri release binary embeds the frontend; WebView2 is fetched on first
; launch via webviewInstallMode: downloadBootstrapper in tauri.conf.json.
Source: "src-tauri\target\release\docforge.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; GroupDescription: "Additional icons:"; Flags: unchecked

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
