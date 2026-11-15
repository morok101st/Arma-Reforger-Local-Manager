; Ergaenzungen zum NSIS-Installer von Tauri. Datei bewusst nur ASCII.
;
; Tauri installiert nach "%LOCALAPPDATA%\<Produktname>", also in einen Ordner mit Leerzeichen und "&".
; Der Pfad geht als Startparameter an das Spiel, deshalb ohne beides, und an dieselbe Stelle wie das ZIP
; laut Anleitung. Eine vorhandene ZIP-Installation wird so gleich ersetzt.
; Die Hooks stehen vor den Definitionen der Vorlage, ${PRODUCTNAME} ist hier noch nicht bekannt.

!define RMS_TAURI_DEFAULT_DIR "$LOCALAPPDATA\Reforger Sync & Launch"
!define RMS_INSTALL_DIR "$LOCALAPPDATA\Programs\ReforgerSyncLaunch"

; Nur der unveraenderte Vorschlag wird ersetzt. Eine fruehere Installation oder ein selbst gewaehlter
; Ordner bleibt, wie er ist.
!macro RMS_DEFAULT_INSTALL_DIR
  ${If} $INSTDIR == "${RMS_TAURI_DEFAULT_DIR}"
    StrCpy $INSTDIR "${RMS_INSTALL_DIR}"
  ${EndIf}
!macroend

; Vor der Ordnerauswahl, damit dort schon der richtige Ordner steht.
!define MUI_CUSTOMFUNCTION_GUIINIT RmsGuiInit
Function RmsGuiInit
  !insertmacro RMS_DEFAULT_INSTALL_DIR
FunctionEnd

; Ohne Oberflaeche (still) laeuft RmsGuiInit nicht.
!macro NSIS_HOOK_PREINSTALL
  !insertmacro RMS_DEFAULT_INSTALL_DIR
  ; Die Vorlage hat den vorgeschlagenen Ordner schon angelegt. Nur leer wird er entfernt.
  ${If} $INSTDIR != "${RMS_TAURI_DEFAULT_DIR}"
    RMDir "${RMS_TAURI_DEFAULT_DIR}"
  ${EndIf}
  SetOutPath $INSTDIR
  ; Rest einer ZIP-Installation. Die Anleitung steht im Installer nicht mehr bei.
  Delete "$INSTDIR\LIES-MICH.txt"
!macroend
