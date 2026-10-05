; Undo capture-exclusion and taskbar hides before the uninstaller deletes the app.
; The injected DLLs stay loaded in other processes, so removing the files alone
; leaves those windows hidden.
!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 hmw_release_done
    DetailPrint "Restoring windows hidden by HideMyWindows..."
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --release-all'
  hmw_release_done:
!macroend
