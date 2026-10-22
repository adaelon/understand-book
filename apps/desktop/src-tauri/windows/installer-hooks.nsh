!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\UnderstandBook" "InstallDir" "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Preserve local libraries, settings, records and user-installed plugins.
  DeleteRegKey HKCU "Software\UnderstandBook"
!macroend
