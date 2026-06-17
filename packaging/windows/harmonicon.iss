; Inno Setup script for Harmonicon — produces a Windows setup .exe.
;
; Built in CI; compile with (paths/version passed in):
;   ISCC.exe /DMyAppVersion=1.2.3 /DStageDir="C:\path\to\stage" packaging\windows\harmonicon.iss
;
; The `stage` directory must contain: harmonicon.exe, icon.ico, LICENSE.txt, and
; an `assets\` folder.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#ifndef StageDir
  #define StageDir "stage"
#endif

#define MyAppName "Harmonicon"
#define MyAppPublisher "Tomaz Canabrava"
#define MyAppURL "https://github.com/tcanabrava/orin"
#define MyAppExeName "harmonicon.exe"

[Setup]
; Stable application identity (do not change between releases).
AppId={{5DC03E3A-1D75-44B1-8769-1289FCBF5780}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile={#StageDir}\LICENSE.txt
OutputDir=installer
OutputBaseFilename=harmonicon-setup-{#MyAppVersion}
SetupIconFile={#StageDir}\icon.ico
UninstallDisplayIcon={app}\icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; 64-bit only (matches the x86_64-pc-windows-msvc build).
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#StageDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
; Game data sits next to the exe so Bevy finds `assets/` from the executable dir.
Source: "{#StageDir}\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; WorkingDir = {app} is important: the game reads some asset files relative to the
; working directory, so shortcuts must launch from the install folder.
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\icon.ico"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
