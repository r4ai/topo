#ifndef AppVersion
  #error AppVersion must be supplied
#endif
#ifndef SourceDir
  #error SourceDir must be supplied
#endif
#ifndef OutputPath
  #error OutputPath must be supplied
#endif

[Setup]
AppId=dev.r4ai.topo
AppName=topo
AppVersion={#AppVersion}
AppPublisher=r4ai
AppPublisherURL=https://github.com/r4ai/topo
DefaultDirName={localappdata}\Programs\topo
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#OutputPath}
OutputBaseFilename=topo-gui-v{#AppVersion}-x86_64-pc-windows-msvc-setup
SetupIconFile=..\..\assets\branding\topo.ico
UninstallDisplayIcon={app}\topo-gui.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceDir}\topo-gui.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\topo"; Filename: "{app}\topo-gui.exe"
Name: "{autodesktop}\topo"; Filename: "{app}\topo-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\topo-gui.exe"; Description: "Launch topo"; Flags: nowait postinstall skipifsilent
