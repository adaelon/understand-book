; Use the same bundled process helper as Tauri's installer. This early check also
; protects upgrades that offer to run an older uninstaller before PREINSTALL.

!macro UNDERSTAND_BOOK_LOCAL_REQUIRE_CLOSED phase
  Push $R0
  Push $R1
  local_close_retry_${phase}:
    !if "${phase}" == "guiinit"
      ; Hooks are included before Tauri registers its process plugin. At this
      ; early GUI entry, query this Windows session without opening a console.
      nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -Command "if (Get-Process -Name UnderstandBookLocal -ErrorAction SilentlyContinue | Where-Object SessionId -eq ([System.Diagnostics.Process]::GetCurrentProcess().SessionId)) { exit 0 }; exit 2"'
      Pop $R0
      Pop $R1
    !else
      nsis_tauri_utils::FindProcessCurrentUser "UnderstandBookLocal.exe"
      Pop $R0
    !endif
    ${If} $R0 = 0
      IfSilent local_close_blocked_${phase}
      ClearErrors
      ${GetOptions} $CMDLINE "/P" $R0
      ${IfNot} ${Errors}
        Goto local_close_blocked_${phase}
      ${EndIf}
      MessageBox MB_ICONEXCLAMATION|MB_RETRYCANCEL "Understand Book Local is still running.$\r$\nClose its windows normally to save your reading state and stop background work, then click Retry.$\r$\n$\r$\n懂书本地版仍在运行。请先正常关闭应用，再点重试继续。" IDRETRY local_close_retry_${phase} IDCANCEL local_close_blocked_${phase}
    ${EndIf}
    Pop $R1
    Pop $R0
    ClearErrors
    Goto local_close_done_${phase}
  local_close_blocked_${phase}:
    Pop $R1
    Pop $R0
    SetErrorLevel 1618
    Quit
  local_close_done_${phase}:
!macroend

!define MUI_CUSTOMFUNCTION_GUIINIT UnderstandBookLocalBeforeInstallUi
Function UnderstandBookLocalBeforeInstallUi
  !insertmacro UNDERSTAND_BOOK_LOCAL_REQUIRE_CLOSED guiinit
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  !insertmacro UNDERSTAND_BOOK_LOCAL_REQUIRE_CLOSED install
!macroend

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\UnderstandBookLocal" "InstallDir" "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro UNDERSTAND_BOOK_LOCAL_REQUIRE_CLOSED uninstall
  ; Tauri's generic checkbox includes the local library and learning records.
  ; This edition always keeps those files after uninstalling the application.
  ${If} $DeleteAppDataCheckboxState = 1
    MessageBox MB_ICONINFORMATION|MB_OKCANCEL "Understand Book Local keeps your library, notes, model settings and learning records when uninstalled.$\r$\nClick OK to uninstall the application and keep your data, or Cancel to keep the application installed.$\r$\n$\r$\n本地版卸载会保留书库、笔记、模型设置和学习记录。点确定继续卸载并保留资料；点取消保留应用。" IDOK local_keep_data IDCANCEL local_cancel_uninstall
    local_cancel_uninstall:
      SetErrorLevel 1602
      Quit
    local_keep_data:
      StrCpy $DeleteAppDataCheckboxState 0
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; User libraries, learning records and external plugins belong to the user.
  DeleteRegKey HKCU "Software\UnderstandBookLocal"
!macroend
