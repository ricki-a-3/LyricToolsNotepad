; LyricToolsNotepad installer (NSIS 3). Per-user install: no admin prompt.
;
; Build (from the rust/ folder, after the release exe is staged):
;   makensis -DVERSION=1.0.0 -DSTAGE=dist\stage -DOUTFILE=dist\LyricToolsNotepad-1.0.0-setup.exe installer\LyricToolsNotepad.nsi
; STAGE must contain LyricToolsNotepad.exe and the license .txt files.

Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !error "Pass -DVERSION=x.y.z"
!endif
!ifndef STAGE
  !error "Pass -DSTAGE=<folder with LyricToolsNotepad.exe>"
!endif
!ifndef OUTFILE
  !define OUTFILE "LyricToolsNotepad-${VERSION}-setup.exe"
!endif

!define APPNAME "LyricToolsNotepad"
!define EXE "LyricToolsNotepad.exe"
!define REGKEY "Software\LyricToolsNotepad"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\LyricToolsNotepad"
; Same key the app's Settings page toggles (src/winshell.rs).
!define SHELLKEY "Software\Classes\SystemFileAssociations\.txt\shell\LyricToolsNotepad"

Name "${APPNAME}"
Caption "${APPNAME} ${VERSION} Setup"
OutFile "${OUTFILE}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\LyricToolsNotepad"
InstallDirRegKey HKCU "${REGKEY}" "InstallDir"
SetCompressor /SOLID lzma
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APPNAME} installer"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ICON "..\assets\start.ico"
!define MUI_UNICON "..\assets\start.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Start ${APPNAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "${APPNAME}" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${STAGE}\${EXE}"
  File "${STAGE}\LICENSE.txt"
  File "${STAGE}\CMUDICT_LICENSE.txt"
  File "${STAGE}\FONT_LICENSE.txt"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${REGKEY}" "InstallDir" "$INSTDIR"

  ; Apps & features entry
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${APPNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" 20000
SectionEnd

Section "Right-click .txt files: Open with ${APPNAME}" SecShell
  WriteRegStr HKCU "${SHELLKEY}" "" "Open with ${APPNAME}"
  WriteRegStr HKCU "${SHELLKEY}" "Icon" '"$INSTDIR\${EXE}",0'
  WriteRegStr HKCU "${SHELLKEY}\command" "" '"$INSTDIR\${EXE}" "%1"'
  ; Tell Explorer the menu changed.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\${EXE}"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "The ${APPNAME} app."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecShell} "Adds $\"Open with ${APPNAME}$\" when you right-click a .txt file."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Puts a ${APPNAME} icon on your desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\CMUDICT_LICENSE.txt"
  Delete "$INSTDIR\FONT_LICENSE.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  Delete "$DESKTOP\${APPNAME}.lnk"
  DeleteRegKey HKCU "${SHELLKEY}"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "${REGKEY}"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ; Your lyrics and settings are left alone.
SectionEnd
