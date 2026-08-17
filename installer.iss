; DocForge Inno Setup installer script
; Builds an updated setup .exe from the current release binary.
; Compile with: "C:\Users\cscha\AppData\Local\Programs\Inno Setup 6\ISCC.exe" installer.iss

#define MyAppName "DocForge"
#define MyAppVersion "2.0.0"
#define MyAppPublisher "DocForge"
#define MyAppExeName "docforge.exe"
#define MyAppId "{E3C7A1B2-9F4D-4C8E-B1A2-7D5E6F3A9C10}"

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
CloseApplications=yes
RestartApplications=no
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

[Code]
(* Automatically remove any previously installed version (and every file/feature it
   installed that is NOT part of this build) before laying down the new files. Inno Setup
   gives each install the same AppId, so the prior uninstaller cleanly deletes all of its
   tracked artifacts; only this build's files remain afterwards. *)
function InitializeSetup(): Boolean;
var
  UninstallKey: string;
  UninstallString: string;
  ResultCode: Integer;
begin
  Result := True;
  UninstallKey := 'Software\Microsoft\Windows\CurrentVersion\Uninstall' + '{{' + '{#MyAppId}' + '}_is1';
  if RegQueryStringValue(HKLM, UninstallKey, 'UninstallString', UninstallString) or
     RegQueryStringValue(HKCU, UninstallKey, 'UninstallString', UninstallString) then
  begin
    UninstallString := RemoveQuotes(UninstallString);
    Exec(UninstallString, '/SILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  end;
end;
