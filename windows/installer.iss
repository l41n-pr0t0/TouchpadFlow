; TouchpadFlow Windows installer (Inno Setup 6). Built by .github/workflows/release.yml:
;   iscc /DArch=x64 /DAppVersion=0.4.0 /DSrcDir=path\to\exes windows\installer.iss
; Installs per user (no admin) to the same place as install.ps1, so either can update the other.

#ifndef Arch
  #define Arch "x64"
#endif
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SrcDir
  #define SrcDir "."
#endif

[Setup]
AppId={{8F3C1B2A-7E4D-4C9A-9B1E-5D2F6A7C3E10}
AppName=TouchpadFlow
AppVersion={#AppVersion}
AppPublisher=l41n-pr0t0
AppPublisherURL=https://github.com/l41n-pr0t0/TouchpadFlow
DefaultDirName={localappdata}\Programs\TouchpadFlow
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputBaseFilename=TouchpadFlow-Setup-{#Arch}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\touchpadflow-app.exe
UninstallDisplayName=TouchpadFlow
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Files]
Source: "{#SrcDir}\TouchpadFlow.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\touchpadflow-app.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\TouchpadFlow\TouchpadFlow"; Filename: "{app}\touchpadflow-app.exe"
Name: "{userprograms}\TouchpadFlow\TouchpadFlow Engine"; Filename: "{app}\TouchpadFlow.exe"

[Registry]
; Same Run value install.ps1 and the engine's own "Start with Windows" option use.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "TouchpadFlow"; ValueData: """{app}\TouchpadFlow.exe"""; Flags: uninsdeletevalue

[Run]
Filename: "{app}\TouchpadFlow.exe"; Description: "Start the TouchpadFlow engine"; Flags: nowait postinstall skipifsilent
Filename: "{app}\touchpadflow-app.exe"; Description: "Open TouchpadFlow settings"; Flags: nowait postinstall skipifsilent unchecked

[UninstallRun]
Filename: "taskkill.exe"; Parameters: "/F /IM TouchpadFlow.exe /IM touchpadflow-app.exe"; Flags: runhidden; RunOnceId: "StopTouchpadFlow"

[Code]
// Running copies lock their .exe files; stop them before files are replaced.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Exec('taskkill.exe', '/F /IM TouchpadFlow.exe /IM touchpadflow-app.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;
