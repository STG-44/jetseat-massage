# -*- coding: utf-8 -*-
Unicode True
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WinVer.nsh"

Name "JetSeat Massage"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\JetSeat Massage"
InstallDirRegKey HKCU "Software\JetSeatMassage" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "${ICON}"
UninstallIcon "${ICON}"
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "JetSeat Massage"
VIAddVersionKey /LANG=1033 "FileDescription" "JetSeat Massage ${ARCH} Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Copyright 2026 JetSeat Massage contributors"
!define MUI_ABORTWARNING
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Russian"

Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_OK|MB_ICONSTOP "JetSeat Massage requires Windows 10 or newer." /SD IDOK
    Abort
  ${EndIf}
  ; An in-app update waits for its own parent to exit before replacing the EXE.
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/UPDATEPID=" $1
  ${IfNot} ${Errors}
    System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i r1) p.r2'
    ${If} $2 != 0
      System::Call 'kernel32::WaitForSingleObject(p r2, i 30000) i.r3'
      System::Call 'kernel32::CloseHandle(p r2)'
      ${If} $3 != 0
        MessageBox MB_OK|MB_ICONSTOP "Close JetSeat Massage and run the installer again." /SD IDOK
        Abort
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

Section "JetSeat Massage" MainSection
  SetShellVarContext current
  SetRegView 64
  SetOutPath "$INSTDIR"
  File /oname=jetseat-massage.exe "${BINARY}"
  File /oname=LICENSE.txt "${LICENSE_FILE}"
  File /oname=README.md "${README_FILE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\JetSeat Massage"
  CreateShortcut "$SMPROGRAMS\JetSeat Massage\JetSeat Massage.lnk" "$INSTDIR\jetseat-massage.exe"
  CreateShortcut "$SMPROGRAMS\JetSeat Massage\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\JetSeatMassage" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "DisplayName" "JetSeat Massage"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "Publisher" "JetSeat Massage contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "DisplayIcon" "$INSTDIR\jetseat-massage.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "QuietUninstallString" '$"$INSTDIR\Uninstall.exe$" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  SetRegView 64
  ClearErrors
  Delete "$INSTDIR\jetseat-massage.exe"
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "Close JetSeat Massage before uninstalling." /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\JetSeat Massage\JetSeat Massage.lnk"
  Delete "$SMPROGRAMS\JetSeat Massage\Uninstall.lnk"
  RMDir "$SMPROGRAMS\JetSeat Massage"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JetSeatMassage"
  DeleteRegKey HKCU "Software\JetSeatMassage"
SectionEnd
