; Inno Setup script for Seen Language installer
; Can be compiled on Linux via: wine ~/.wine/drive_c/Program\ Files\ \(x86\)/Inno\ Setup\ 6/ISCC.exe seen.iss
; Or on Windows: iscc seen.iss
;
; Prerequisites: Run package_windows.sh first to create the staging directory.

#ifndef Version
  #define Version "1.0.0"
#endif
#ifndef SourceDir
  ; Use forward slashes for Wine compatibility on Linux; Inno Setup accepts both
  #define SourceDir "../../target-windows/seen-" + Version + "-windows-x64"
#endif

[Setup]
AppName=Seen Language
AppVersion={#Version}
AppVerName=Seen Language {#Version}
AppPublisher=Seen Language Team
AppPublisherURL=https://github.com/codeyousef/SeenLang
AppSupportURL=https://github.com/codeyousef/SeenLang/issues
DefaultDirName={autopf}\Seen
DefaultGroupName=Seen Language
OutputBaseFilename=Seen-{#Version}-windows-x64-setup
OutputDir=output
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
MinVersion=10.0
PrivilegesRequired=admin
WizardStyle=modern
SetupIconFile=..\..\installer\assets\icons\seen-icon.ico
; UninstallDisplayIcon={app}\bin\seen.exe
LicenseFile=..\..\LICENSE

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Full installation"
Name: "compact"; Description: "Compiler only"
Name: "custom"; Description: "Custom installation"; Flags: iscustom

[Components]
Name: "compiler"; Description: "Seen Compiler (seen.exe)"; Types: full compact custom; Flags: fixed
Name: "runtime"; Description: "Runtime Headers (for FFI)"; Types: full
Name: "stdlib"; Description: "Standard Library"; Types: full
Name: "languages"; Description: "Multi-language Keywords (6 languages)"; Types: full
Name: "docs"; Description: "Documentation"; Types: full

[Files]
; Compiler binary
Source: "{#SourceDir}\bin\seen.exe"; DestDir: "{app}\bin"; Components: compiler; Flags: ignoreversion

; Runtime headers
Source: "{#SourceDir}\lib\seen\runtime\*.h"; DestDir: "{app}\lib\seen\runtime"; Components: runtime; Flags: ignoreversion

; Standard library (recursive)
Source: "{#SourceDir}\lib\seen\std\*"; DestDir: "{app}\lib\seen\std"; Components: stdlib; Flags: ignoreversion recursesubdirs createallsubdirs

; Language configurations (recursive)
Source: "{#SourceDir}\share\seen\languages\*"; DestDir: "{app}\share\seen\languages"; Components: languages; Flags: ignoreversion recursesubdirs createallsubdirs

; Documentation
Source: "{#SourceDir}\share\seen\docs\*"; DestDir: "{app}\share\seen\docs"; Components: docs; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist

; License and README
Source: "{#SourceDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Dirs]
Name: "{app}\bin"
Name: "{app}\lib\seen\runtime"
Name: "{app}\lib\seen\std"
Name: "{app}\share\seen\languages"
Name: "{app}\share\seen\docs"

[Icons]
Name: "{group}\Seen Command Prompt"; Filename: "{cmd}"; Parameters: "/k echo Seen Language Environment && seen compile --help"; WorkingDir: "{userdocs}"
Name: "{group}\Uninstall Seen"; Filename: "{uninstallexe}"

[Registry]
; Install path for tool discovery
Root: HKLM; Subkey: "SOFTWARE\SeenLanguage"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\SeenLanguage"; ValueType: string; ValueName: "Version"; ValueData: "{#Version}"
Root: HKLM; Subkey: "SOFTWARE\SeenLanguage"; ValueType: string; ValueName: "BinPath"; ValueData: "{app}\bin"

; App Paths (allows running "seen" from Run dialog)
Root: HKLM; Subkey: "SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\seen.exe"; ValueType: string; ValueData: "{app}\bin\seen.exe"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\seen.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}\bin"

; File association for .seen files
Root: HKCR; Subkey: ".seen"; ValueType: string; ValueData: "SeenSourceFile"; Flags: uninsdeletevalue
Root: HKCR; Subkey: "SeenSourceFile"; ValueType: string; ValueData: "Seen Source File"; Flags: uninsdeletekey
Root: HKCR; Subkey: "SeenSourceFile\shell\open\command"; ValueType: string; ValueData: """{app}\bin\seen.exe"" ""%1"""

[Tasks]
Name: "addtopath"; Description: "Add Seen to system PATH"; GroupDescription: "Environment:"
Name: "fileassoc"; Description: "Associate .seen files with Seen compiler"; GroupDescription: "File associations:"

[Code]
// Add bin directory to system PATH
procedure CurStepChanged(CurStep: TSetupStep);
var
  Path: string;
  BinDir: string;
begin
  if CurStep = ssPostInstall then
  begin
    if IsTaskSelected('addtopath') then
    begin
      BinDir := ExpandConstant('{app}\bin');
      if RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path) then
      begin
        if Pos(Lowercase(BinDir), Lowercase(Path)) = 0 then
        begin
          Path := Path + ';' + BinDir;
          RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
        end;
      end;
    end;
  end;
end;

// Remove bin directory from PATH on uninstall
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path: string;
  BinDir: string;
  P: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    BinDir := ExpandConstant('{app}\bin');
    if RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path) then
    begin
      P := Pos(';' + Lowercase(BinDir), Lowercase(Path));
      if P > 0 then
      begin
        Delete(Path, P, Length(BinDir) + 1);
        RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
      end
      else
      begin
        P := Pos(Lowercase(BinDir) + ';', Lowercase(Path));
        if P > 0 then
        begin
          Delete(Path, P, Length(BinDir) + 1);
          RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
        end
        else
        begin
          P := Pos(Lowercase(BinDir), Lowercase(Path));
          if P > 0 then
          begin
            Delete(Path, P, Length(BinDir));
            RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
          end;
        end;
      end;
    end;
  end;
end;

// Check for LLVM after installation
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if CurPageID = wpFinished then
  begin
    // Could check for LLVM here and warn if missing
  end;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpFinished then
  begin
    WizardForm.FinishedLabel.Caption :=
      'Seen Language has been installed.' + #13#10 + #13#10 +
      'Open a new terminal and run:' + #13#10 +
      '  seen compile hello.seen hello --fast' + #13#10 + #13#10 +
      'Note: LLVM (clang, opt) must be installed and in PATH for compilation.' + #13#10 +
      'Install via: winget install LLVM.LLVM';
  end;
end;
