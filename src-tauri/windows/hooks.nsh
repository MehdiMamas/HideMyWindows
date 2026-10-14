; Payload files and other apps
; -----------------------------
; Apps keep the payload DLL loaded after HideMyWindows stops, and Windows will
; not delete or overwrite a loaded DLL. Windows also stores the window gate's
; global hook as (DLL path, offset). Up to 2.3.1 every build used
; resources\hmw_payload.dll, so an app that still had an older build loaded
; got that build back for the new hook and ran the new offset inside old code,
; which crashed it.
;
; Since 2.3.2 each build lives in resources\payload\<content id>\ and no path
; ever holds different code. These hooks only tidy up: they delete payload
; files nothing has loaded, rename loaded ones out of the way (an app keeps
; using its renamed copy) and, when the installer runs as administrator, have
; Windows delete them at the next restart. Nothing here closes another app.

!include LogicLib.nsh

; Delete $0 at the next restart. Only works when running as administrator.
!macro HMW_DELETE_AT_REBOOT
  System::Call 'kernel32::MoveFileExW(w "$0", p 0, i 4) i .r4'
!macroend

; Delete $0, or rename it out of the way when an app still has it loaded.
!macro HMW_REMOVE_PAYLOAD_FILE
  ${If} ${FileExists} "$0"
    ClearErrors
    Delete "$0"
    ${If} ${Errors}
      System::Call 'kernel32::GetTickCount() i .r4'
      ClearErrors
      Rename "$0" "$0.$4.old"
      ${IfNot} ${Errors}
        StrCpy $0 "$0.$4.old"
      ${EndIf}
      !insertmacro HMW_DELETE_AT_REBOOT
    ${EndIf}
  ${EndIf}
!macroend

; Delete every `hmw_payload.dll*.old` in folder $3.
!macro HMW_REMOVE_OLD_COPIES
  FindFirst $1 $2 "$3\hmw_payload.dll*.old"
  ${DoWhile} $2 != ""
    StrCpy $0 "$3\$2"
    ClearErrors
    Delete "$0"
    ${If} ${Errors}
      !insertmacro HMW_DELETE_AT_REBOOT
    ${EndIf}
    FindNext $1 $2
  ${Loop}
  FindClose $1
!macroend

; Every payload file of every earlier build, and their folders once empty.
!macro HMW_SWEEP_PAYLOADS
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  ; The shared path used up to 2.3.1, and copies renamed by earlier updates.
  StrCpy $3 "$INSTDIR\resources"
  !insertmacro HMW_REMOVE_OLD_COPIES
  StrCpy $0 "$INSTDIR\resources\hmw_payload.dll"
  !insertmacro HMW_REMOVE_PAYLOAD_FILE
  ; Per-build folders since 2.3.2. A reinstall of the same build writes the
  ; same path again; renaming a loaded copy is safe because the new file has
  ; the same content.
  FindFirst $5 $6 "$INSTDIR\resources\payload\*"
  ${DoWhile} $6 != ""
    ${If} $6 != "."
    ${AndIf} $6 != ".."
      StrCpy $3 "$INSTDIR\resources\payload\$6"
      !insertmacro HMW_REMOVE_OLD_COPIES
      StrCpy $0 "$3\hmw_payload.dll"
      !insertmacro HMW_REMOVE_PAYLOAD_FILE
      RMDir "$3"
    ${EndIf}
    FindNext $5 $6
  ${Loop}
  FindClose $5
  RMDir "$INSTDIR\resources\payload"
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro HMW_SWEEP_PAYLOADS
!macroend

; Undo capture-exclusion and taskbar hides before the uninstaller deletes the app.
; The injected DLLs stay loaded in other processes, so removing the files alone
; leaves those windows hidden. This calls each loaded copy's own exports; it
; never unloads a DLL or closes an app.
!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 hmw_release_done
    DetailPrint "Restoring windows hidden by HideMyWindows..."
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --release-all'
  hmw_release_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro HMW_SWEEP_PAYLOADS
  RMDir "$INSTDIR\resources"
  RMDir "$INSTDIR"
!macroend
