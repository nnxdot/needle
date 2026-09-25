; Needle's Windows installer (Inno Setup 6). Built by scripts\build-windows.ps1, which passes
; the version and the folder with the release build:
;   ISCC.exe /DAppVersion=1.0.0 /DSource=..\dist\Needle packaging\needle.iss
;
; Installs for the current user (no administrator needed), adds a Start menu entry, and offers
; Needle in "Open with" for music files without taking over the default player.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Source
  #define Source "..\dist\Needle"
#endif

[Setup]
AppId={{6E3B1F0A-1D1B-4C3B-9E36-7F1E2B6C9A51}
AppName=Needle
AppVersion={#AppVersion}
AppVerName=Needle {#AppVersion}
AppPublisher=Needle
AppPublisherURL=https://needle.nnx.fyi/
AppSupportURL=mailto:dot@nnx.fyi
AppUpdatesURL=https://needle.nnx.fyi/#download
DefaultDirName={localappdata}\Programs\Needle
DefaultGroupName=Needle
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Needle-Setup-{#AppVersion}
SetupIconFile=..\crates\needle\assets\needle.ico
UninstallDisplayIcon={app}\Needle.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Close a running Needle before replacing it (updates run this quietly), by force if it does
; not close by itself. See also PrepareToInstall below.
CloseApplications=force
RestartApplications=no
ChangesAssociations=yes

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Needle"; Filename: "{app}\Needle.exe"
Name: "{autodesktop}\Needle"; Filename: "{app}\Needle.exe"; Tasks: desktopicon

[Registry]
; The file type Needle opens.
Root: HKA; Subkey: "Software\Classes\Needle.AudioFile"; ValueType: string; ValueData: "Music file"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Needle.AudioFile\DefaultIcon"; ValueType: string; ValueData: "{app}\Needle.exe,0"
Root: HKA; Subkey: "Software\Classes\Needle.AudioFile\shell\open\command"; ValueType: string; ValueData: """{app}\Needle.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\shell\open\command"; ValueType: string; ValueData: """{app}\Needle.exe"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Needle"
; "Open with" for each music type, without becoming the default.
Root: HKA; Subkey: "Software\Classes\.flac\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".flac"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".flac"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.mp3\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".mp3"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".mp3"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.m4a\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".m4a"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".m4a"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.aac\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".aac"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".aac"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.ogg\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".ogg"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".ogg"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.opus\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".opus"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".opus"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.wav\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".wav"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".wav"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.aiff\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".aiff"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".aiff"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.aif\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".aif"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".aif"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.wv\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".wv"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".wv"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.ape\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".ape"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".ape"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.dsf\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".dsf"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".dsf"; ValueData: "Needle.AudioFile"
Root: HKA; Subkey: "Software\Classes\.cue\OpenWithProgids"; ValueType: string; ValueName: "Needle.AudioFile"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\Needle.exe\SupportedTypes"; ValueType: string; ValueName: ".cue"; ValueData: ""
Root: HKA; Subkey: "Software\Needle\Capabilities\FileAssociations"; ValueType: string; ValueName: ".cue"; ValueData: "Needle.AudioFile"
; Listed in Settings › Apps › Default apps.
Root: HKA; Subkey: "Software\Needle\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Needle"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Needle\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Your music, in its place"
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "Needle"; ValueData: "Software\Needle\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\Needle.exe"; Description: "{cm:LaunchProgram,Needle}"; Flags: nowait postinstall

[Code]
function GetTickCount: DWord; external 'GetTickCount@kernel32.dll stdcall';

{ The Needle.exe processes running from the install folder, as a WMI result set: live
  Win32_Process objects, each bound to its own process. }
function RunningNeedles(): Variant;
var
  Path: String;
  Locator, Service: Variant;
begin
  Path := ExpandConstant('{app}\Needle.exe');
  StringChangeEx(Path, '\', '\\', True);
  StringChangeEx(Path, '''', '\''', True);
  Locator := CreateOleObject('WbemScripting.SWbemLocator');
  { 128: wbemConnectFlagUseMaxWait, so a WMI that does not answer fails instead of hanging. }
  Service := Locator.ConnectServer('.', 'root\CIMV2', '', '', '', '', 128);
  Result := Service.ExecQuery('SELECT Handle, ProcessId FROM Win32_Process WHERE ExecutablePath = ''' +
    Path + '''');
end;

{ An update runs quietly just after Needle asked to close. Needle 1.5.0 can take half a
  minute to close when its music server does not answer, and the Restart Manager then waits
  on it. So a quiet install gives Needle 8 seconds, and then ends what is left. }
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Found: Variant;
  I: Integer;
  Started: DWord;
begin
  Result := '';
  if not WizardSilent then
    exit;
  try
    Started := GetTickCount;
    Found := RunningNeedles();
    while (Found.Count > 0) and (GetTickCount - Started < 8000) do
    begin
      Sleep(250);
      Found := RunningNeedles();
    end;
    { A fresh look right before, so only a Needle from this folder that still runs is ended,
      never another program that got a finished Needle's process number. }
    Found := RunningNeedles();
    for I := 0 to Found.Count - 1 do
      Found.ItemIndex(I).Terminate(1);
  except
    { Without WMI, the Restart Manager still closes Needle, by force if it must. }
  end;
end;
