; The payload stays loaded in hidden apps, so Windows will not overwrite the
; DLL in place. Renaming it lets NSIS write the new file. Processes that
; already injected keep the renamed copy mapped, and the new build injects
; the replacement when rules reapply.
!macro NSIS_HOOK_PREINSTALL
  Push $0
  Push $1
  StrCpy $0 "$INSTDIR\resources\hmw_payload.dll"
  IfFileExists "$0" 0 hmw_payload_rename_done
    ClearErrors
    Delete "$0.old"
    ClearErrors
    Rename "$0" "$0.old"
    IfErrors 0 hmw_payload_rename_done
      ; A previous update's .old is still mapped. Move the live file aside.
      System::Call 'kernel32::GetTickCount() i .r1'
      ClearErrors
      Rename "$0" "$0.$1.old"
  hmw_payload_rename_done:
  Pop $1
  Pop $0
!macroend

; Undo capture-exclusion and taskbar hides before the uninstaller deletes the app.
; The injected DLLs stay loaded in other processes, so removing the files alone
; leaves those windows hidden.
!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 hmw_release_done
    DetailPrint "Restoring windows hidden by HideMyWindows..."
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --release-all'
  hmw_release_done:
!macroend
