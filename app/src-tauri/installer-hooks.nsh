; Idea Autopilot keeps ideas and settings in its own data folder (portable layout next to the
; program, or %USERPROFILE%\IdeaAutopilot for an installed copy) — never inside the install
; folder — so updating or uninstalling does not touch them. These hooks only clean up what the
; program itself registered with Windows.

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Idea Autopilot"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\app.ideaautopilot.desktop"
!macroend
