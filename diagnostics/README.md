# WhatsApp window-state recorder

Use this when a protected WhatsApp window leaves a local rectangle after
minimizing while another app has focus. The rectangle may be absent from screen
captures, so this records window metadata instead.

1. Download `WhatsApp-window-check.ps1` to a writable folder.
2. Open PowerShell in that folder and run:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File .\WhatsApp-window-check.ps1
   ```

3. Press Enter. Switch to another app and minimize the protected WhatsApp window
   the same way that normally causes the rectangle. Leave the rectangle in place
   until the 20-second recording finishes if possible.
4. Send the generated `WhatsApp-window-state-*.json` file with the installed
   HideMyWindows and WhatsApp versions, and whether the rectangle appeared.

The script does not take screenshots, read window titles or chats, change window
state, or transmit data. It records process names and IDs, HWNDs, window classes,
rectangles, foreground changes, capture affinity, DWM cloak state, and
HideMyWindows diagnostic markers. Review the JSON before sharing it.

`CloakResult` is zero when no attempt was recorded. Otherwise subtract one and
interpret the low 32 bits as an HRESULT; one represents a successful API call.
These extra markers are available in diagnostic candidates after 2.1.1.
