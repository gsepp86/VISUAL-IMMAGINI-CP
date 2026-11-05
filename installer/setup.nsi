; Installer di Visualizzatore Immagini CP (NSIS 3.x, Unicode)
; Compilazione:  makensis installer\setup.nsi   (dalla cartella del progetto, dopo "cargo build --release")
Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
!insertmacro GetSize

!define APP_NAME "Visualizzatore Immagini CP"
!define APP_EXE "visualizzatore-immagini-cp.exe"
!define APP_VERSION "0.1.0"
!define APP_PUBLISHER "Peppe CP"
!define PROGID "VisualizzatoreImmaginiCP.Image"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\VisualizzatoreImmaginiCP"

Name "${APP_NAME}"
OutFile "..\dist\Setup-Visualizzatore-Immagini-CP-${APP_VERSION}.exe"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "Software\VisualizzatoreImmaginiCP" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "${APP_NAME} - ${APP_PUBLISHER}"

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Avvia ${APP_NAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Italian"

; Programma a 64 bit: registro a 64 bit e collegamenti per tutti gli utenti
Function .onInit
  SetRegView 64
  SetShellVarContext all
FunctionEnd

Function un.onInit
  SetRegView 64
  SetShellVarContext all
FunctionEnd

; Estensioni associate
!macro ForEachExt MACRO
  !insertmacro "${MACRO}" ".jpg"
  !insertmacro "${MACRO}" ".jpeg"
  !insertmacro "${MACRO}" ".jpe"
  !insertmacro "${MACRO}" ".jfif"
  !insertmacro "${MACRO}" ".png"
  !insertmacro "${MACRO}" ".gif"
  !insertmacro "${MACRO}" ".bmp"
  !insertmacro "${MACRO}" ".dib"
  !insertmacro "${MACRO}" ".ico"
  !insertmacro "${MACRO}" ".tif"
  !insertmacro "${MACRO}" ".tiff"
  !insertmacro "${MACRO}" ".webp"
  !insertmacro "${MACRO}" ".tga"
  !insertmacro "${MACRO}" ".dds"
  !insertmacro "${MACRO}" ".hdr"
  !insertmacro "${MACRO}" ".exr"
  !insertmacro "${MACRO}" ".pnm"
  !insertmacro "${MACRO}" ".pbm"
  !insertmacro "${MACRO}" ".pgm"
  !insertmacro "${MACRO}" ".ppm"
  !insertmacro "${MACRO}" ".qoi"
  !insertmacro "${MACRO}" ".svg"
  !insertmacro "${MACRO}" ".heic"
  !insertmacro "${MACRO}" ".heif"
  !insertmacro "${MACRO}" ".avif"
  !insertmacro "${MACRO}" ".jxl"
  !insertmacro "${MACRO}" ".cr2"
  !insertmacro "${MACRO}" ".cr3"
  !insertmacro "${MACRO}" ".nef"
  !insertmacro "${MACRO}" ".arw"
  !insertmacro "${MACRO}" ".dng"
  !insertmacro "${MACRO}" ".orf"
  !insertmacro "${MACRO}" ".rw2"
  !insertmacro "${MACRO}" ".raf"
  !insertmacro "${MACRO}" ".pef"
  !insertmacro "${MACRO}" ".srw"
!macroend

!macro AddExt EXT
  WriteRegStr HKLM "Software\VisualizzatoreImmaginiCP\Capabilities\FileAssociations" "${EXT}" "${PROGID}"
  WriteRegStr HKLM "Software\Classes\${EXT}\OpenWithProgids" "${PROGID}" ""
  ; Menu destro di Esplora risorse: "Stampa con ..." anche su più file selezionati
  WriteRegStr HKLM "Software\Classes\SystemFileAssociations\${EXT}\shell\VICPrint" "" "Stampa con ${APP_NAME}"
  WriteRegStr HKLM "Software\Classes\SystemFileAssociations\${EXT}\shell\VICPrint\command" "" '"$INSTDIR\${APP_EXE}" --print "%1"'
!macroend

!macro DelExt EXT
  DeleteRegValue HKLM "Software\Classes\${EXT}\OpenWithProgids" "${PROGID}"
  DeleteRegKey HKLM "Software\Classes\SystemFileAssociations\${EXT}\shell\VICPrint"
!macroend

Section "${APP_NAME}" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "..\target\release\${APP_EXE}"
  File "..\assets\icon.ico"
  File "..\LICENSE"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Disinstallazione da "App installate"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\icon.ico"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" "$0"
  WriteRegStr HKLM "Software\VisualizzatoreImmaginiCP" "InstallDir" "$INSTDIR"

  ; Come si apre un'immagine
  WriteRegStr HKLM "Software\Classes\${PROGID}" "" "Immagine"
  WriteRegStr HKLM "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\${APP_EXE},0"
  WriteRegStr HKLM "Software\Classes\${PROGID}\shell\open" "FriendlyAppName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\${APP_EXE}" "%1"'

  ; Registrazione tra le app predefinite di Windows
  WriteRegStr HKLM "Software\Classes\Applications\${APP_EXE}" "FriendlyAppName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Classes\Applications\${APP_EXE}\shell\open\command" "" '"$INSTDIR\${APP_EXE}" "%1"'
  WriteRegStr HKLM "Software\VisualizzatoreImmaginiCP\Capabilities" "ApplicationName" "${APP_NAME}"
  WriteRegStr HKLM "Software\VisualizzatoreImmaginiCP\Capabilities" "ApplicationDescription" "Lettore immagini opensource creato da Peppe CP in collaborazione con IA"
  WriteRegStr HKLM "Software\RegisteredApplications" "VisualizzatoreImmaginiCP" "Software\VisualizzatoreImmaginiCP\Capabilities"
  !insertmacro ForEachExt AddExt

  ; Dice a Esplora risorse di aggiornare le associazioni
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Collegamento nel menu Start" SecStart
  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\icon.ico"
SectionEnd

Section "Collegamento sul Desktop" SecDesktop
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\icon.ico"
SectionEnd

Section /o "Apri Impostazioni per scegliere il programma predefinito" SecDefault
  ; Windows 10/11 non permette ai programmi di impostarsi da soli come predefiniti: apriamo la pagina giusta
  ExecShell "open" "ms-settings:defaultapps"
SectionEnd

LangString DESC_Main ${LANG_ITALIAN} "Programma principale (obbligatorio)."
LangString DESC_Start ${LANG_ITALIAN} "Crea un collegamento nel menu Start."
LangString DESC_Desktop ${LANG_ITALIAN} "Crea un collegamento sul Desktop."
LangString DESC_Default ${LANG_ITALIAN} "Al termine apre le Impostazioni di Windows per scegliere ${APP_NAME} come programma predefinito per le immagini."
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} $(DESC_Main)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecStart} $(DESC_Start)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_Desktop)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDefault} $(DESC_Default)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  !insertmacro ForEachExt DelExt
  DeleteRegValue HKLM "Software\RegisteredApplications" "VisualizzatoreImmaginiCP"
  DeleteRegKey HKLM "Software\VisualizzatoreImmaginiCP"
  DeleteRegKey HKLM "Software\Classes\${PROGID}"
  DeleteRegKey HKLM "Software\Classes\Applications\${APP_EXE}"
  DeleteRegKey HKLM "${UNINST_KEY}"

  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd
