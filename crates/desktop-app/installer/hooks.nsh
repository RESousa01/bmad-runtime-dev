; Forge installer hooks.
;
; Wired up through tauri.conf.json > bundle > windows > nsis > installerHooks.
; Tauri includes this file near the top of the generated installer.nsi and
; expands each macro, if defined, at a fixed point in the install or uninstall
; section. Only the hooks Forge actually needs are defined; the generated
; script guards each one with !ifmacrodef.

; NSIS_HOOK_PREUNINSTALL -- runs at the top of Section Uninstall, after the
; uninstall confirmation page has recorded the user's choice and before
; anything is removed.
;
; Why this exists: the stock uninstall confirmation page carries a checkbox
; that, when ticked, runs `RmDir /r "$LOCALAPPDATA\<identifier>"` at the end
; of the section. For Forge that directory is the encrypted local authority
; store -- chat transcripts, approval decisions, capability results, and every
; workspace grant. Inside the app, destroying exactly that store through
; `app.offboarding.erase` requires typing an exact confirmation phrase. A
; single stray click in an uninstaller should not be a weaker gate than the
; app's own, so this asks once more, spells out what is about to be lost, and
; treats anything other than an explicit Yes as "keep my data".
;
; Everything is guarded by ${Silent}. `tools/qualify-windows-installer.ps1`
; drives uninstall with /S and fails if the process does not exit cleanly, so
; this must never raise a dialog in that path. It cannot: a silent uninstall
; never shows the confirmation page, so $DeleteAppDataCheckboxState stays 0
; and a silent uninstall always keeps the local data.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIfNot} ${Silent}
    MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "$(forgeEraseLocalDataConfirm)" /SD IDNO IDYES +2
    StrCpy $DeleteAppDataCheckboxState 0
  ${EndIf}
!macroend
