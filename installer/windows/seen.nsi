; NSIS installer for Seen Language
;
; Build on Linux:  makensis -DVERSION=1.0.0 -DSOURCE_DIR=path/to/staged seen.nsi
; Build on Windows: makensis /DVERSION=1.0.0 /DSOURCE_DIR=path\to\staged seen.nsi
;
; Install NSIS:
;   Arch Linux: sudo pacman -S nsis
;   Ubuntu:     sudo apt-get install nsis
;   Windows:    https://nsis.sourceforge.io/Download

!ifndef VERSION
  !define VERSION "1.0.0"
!endif
!ifndef SOURCE_DIR
  !define SOURCE_DIR "../../target-windows/seen-${VERSION}-windows-x64"
!endif

!define PRODUCT_NAME "Seen Language"
!define PRODUCT_PUBLISHER "Seen Language Team"
!define PRODUCT_WEB_SITE "https://github.com/codeyousef/SeenLang"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\SeenLanguage"
!define REG_KEY "Software\SeenLanguage"
!define ENV_KEY "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"

!include "MUI2.nsh"
!include "WinMessages.nsh"
!include "LogicLib.nsh"
!include "WordFunc.nsh"

Name "${PRODUCT_NAME} ${VERSION}"
OutFile "output/Seen-${VERSION}-windows-x64-setup.exe"
InstallDir "$PROGRAMFILES64\Seen"
InstallDirRegKey HKLM "${REG_KEY}" "InstallPath"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "${PRODUCT_NAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "${PRODUCT_PUBLISHER}"
VIAddVersionKey "FileDescription" "${PRODUCT_NAME} Installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Welcome to ${PRODUCT_NAME} Setup"
!define MUI_WELCOMEPAGE_TEXT "This will install the Seen compiler and toolchain.$\r$\n$\r$\nSeen supports keywords in 6 languages: English, Arabic, Spanish, Russian, Chinese, Japanese.$\r$\n$\r$\nClick Next to continue."
!define MUI_FINISHPAGE_TITLE "Installation Complete"
!define MUI_FINISHPAGE_TEXT "Open a new terminal and run:$\r$\n  seen compile hello.seen hello --fast$\r$\n$\r$\nLLVM is required for compilation:$\r$\n  winget install LLVM.LLVM"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "../../LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; ========================= Install =========================

Section "!Seen Compiler (required)" SEC_COMPILER
  SectionIn RO
  SetOutPath "$INSTDIR\bin"
  File "${SOURCE_DIR}/bin/seen.exe"
  SetOutPath "$INSTDIR"
  File "${SOURCE_DIR}/README.txt"
  File /oname=LICENSE.txt "../../LICENSE"

  ; Registry
  WriteRegStr HKLM "${REG_KEY}" "InstallPath" "$INSTDIR"
  WriteRegStr HKLM "${REG_KEY}" "Version" "${VERSION}"
  WriteRegStr HKLM "${REG_KEY}" "BinPath" "$INSTDIR\bin"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\App Paths\seen.exe" "" "$INSTDIR\bin\seen.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\App Paths\seen.exe" "Path" "$INSTDIR\bin"

  ; Uninstaller + Add/Remove Programs
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr   HKLM "${UNINST_KEY}" "DisplayName" "${PRODUCT_NAME} ${VERSION}"
  WriteRegStr   HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr   HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr   HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr   HKLM "${UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr   HKLM "${UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
  WriteRegStr   HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Add to system PATH" SEC_PATH
  ; Append $INSTDIR\bin to system PATH
  ReadRegStr $0 HKLM "${ENV_KEY}" "Path"
  ${If} $0 == ""
    WriteRegExpandStr HKLM "${ENV_KEY}" "Path" "$INSTDIR\bin"
  ${Else}
    WriteRegExpandStr HKLM "${ENV_KEY}" "Path" "$0;$INSTDIR\bin"
  ${EndIf}
  ; Save our bin path for uninstaller
  WriteRegStr HKLM "${REG_KEY}" "AddedToPath" "$INSTDIR\bin"
  ; Broadcast environment change
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=500
SectionEnd

Section "Runtime Headers" SEC_RUNTIME
  SetOutPath "$INSTDIR\lib\seen\runtime"
  File /nonfatal "${SOURCE_DIR}/lib/seen/runtime/*.h"
SectionEnd

Section "Standard Library" SEC_STDLIB
  SetOutPath "$INSTDIR\lib\seen\std"
  File /nonfatal /r "${SOURCE_DIR}/lib/seen/std/*.*"
SectionEnd

Section "Multi-language Keywords" SEC_LANGUAGES
  SetOutPath "$INSTDIR\share\seen\languages"
  File /nonfatal /r "${SOURCE_DIR}/share/seen/languages/*.*"
SectionEnd

Section /o "Documentation" SEC_DOCS
  SetOutPath "$INSTDIR\share\seen\docs"
  File /nonfatal /r "${SOURCE_DIR}/share/seen/docs/*.*"
SectionEnd

Section ".seen File Association" SEC_FILEASSOC
  WriteRegStr HKCR ".seen" "" "SeenSourceFile"
  WriteRegStr HKCR "SeenSourceFile" "" "Seen Source File"
  WriteRegStr HKCR "SeenSourceFile\shell\open\command" "" '"$INSTDIR\bin\seen.exe" "%1"'
SectionEnd

Section "Start Menu Shortcuts" SEC_STARTMENU
  CreateDirectory "$SMPROGRAMS\Seen Language"
  CreateShortcut "$SMPROGRAMS\Seen Language\Seen Command Prompt.lnk" \
    "$SYSDIR\cmd.exe" '/k "echo. & echo   Seen Language Environment & echo."'
  CreateShortcut "$SMPROGRAMS\Seen Language\Uninstall Seen.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

; Component descriptions
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_COMPILER}  "The Seen compiler (seen.exe). Required."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_PATH}       "Add Seen to system PATH for terminal access."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_RUNTIME}    "C runtime headers for FFI development."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_STDLIB}     "Standard library: collections, io, sync, async, SIMD, GPU, JSON, FFI."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_LANGUAGES}  "Keywords in English, Arabic, Spanish, Russian, Chinese, Japanese."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_DOCS}       "Language guide and API reference."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_FILEASSOC}  "Associate .seen files with the Seen compiler."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_STARTMENU}  "Create Start Menu shortcuts."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; ========================= Uninstall =========================

Section "Uninstall"
  ; Remove files
  RMDir /r "$INSTDIR\bin"
  RMDir /r "$INSTDIR\lib"
  RMDir /r "$INSTDIR\share"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  ; Remove from PATH: read what we added, remove it
  ReadRegStr $1 HKLM "${REG_KEY}" "AddedToPath"
  ${If} $1 != ""
    ReadRegStr $0 HKLM "${ENV_KEY}" "Path"
    ; Remove ";$1" from PATH (entry after another)
    ${WordReplace} $0 ";$1" "" "+" $0
    ; Remove "$1;" from PATH (if it was first entry with others after)
    ${WordReplace} $0 "$1;" "" "+" $0
    ; Remove "$1" alone only if it's the entire PATH value (exact match)
    ${If} $0 == "$1"
      StrCpy $0 ""
    ${EndIf}
    WriteRegExpandStr HKLM "${ENV_KEY}" "Path" "$0"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=500
  ${EndIf}

  ; Remove registry
  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "${REG_KEY}"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\App Paths\seen.exe"
  DeleteRegKey HKCR ".seen"
  DeleteRegKey HKCR "SeenSourceFile"

  ; Remove Start Menu
  RMDir /r "$SMPROGRAMS\Seen Language"
SectionEnd
