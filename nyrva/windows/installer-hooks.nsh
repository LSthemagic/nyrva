; Keep the per-user Nyrva install directory available to terminal users.
; Tauri's default NSIS mode installs under LocalAppData, so no elevation is
; required and only HKCU is modified.

!include "WinMessages.nsh"

!macro NYRVA_ADD_USER_PATH
  ReadRegStr $0 HKCU "Environment" "Path"
  StrCmp $0 "" nyrva_path_empty

  ; Do not append a duplicate entry on repair/reinstall.
  Push $0
  Push "$INSTDIR"
  Call NyrvaPathContains
  Pop $1
  StrCmp $1 "1" nyrva_path_notify

  WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
  Goto nyrva_path_notify

nyrva_path_empty:
  WriteRegExpandStr HKCU "Environment" "Path" "$INSTDIR"

nyrva_path_notify:
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NYRVA_REMOVE_USER_PATH
  ReadRegStr $0 HKCU "Environment" "Path"
  StrCmp $0 "" nyrva_remove_done

  Push $0
  Push "$INSTDIR"
  Call un.NyrvaRemovePathEntry
  Pop $1
  StrCmp $1 $0 nyrva_remove_done
  WriteRegExpandStr HKCU "Environment" "Path" "$1"
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000

nyrva_remove_done:
!macroend

; Stack input: full PATH, entry. Stack output: "1" if an exact
; semicolon-delimited entry exists, otherwise "0".
Function NyrvaPathContains
  Exch $1
  Exch
  Exch $0
  Push $2
  Push $3
  StrCpy $2 ";$0;"
  StrCpy $3 ";$1;"
  Push $2
  Push $3
  Call NyrvaStringContains
  Pop $2
  Pop $3
  StrCmp $2 "1" 0 +2
    StrCpy $3 "1"
  StrCmp $2 "1" +2 0
    StrCpy $3 "0"
  Pop $2
  Pop $1
  Exch $3
FunctionEnd

; Stack input: haystack, needle. Stack output: "1"/"0".
Function NyrvaStringContains
  Exch $1
  Exch
  Exch $0
  Push $2
  Push $3
  Push $4
  StrLen $2 $1
  StrLen $3 $0
  StrCpy $4 0
nyrva_contains_loop:
  IntCmp $4 $3 nyrva_contains_no nyrva_contains_check nyrva_contains_no
nyrva_contains_check:
  StrCpy $R0 $0 $2 $4
  StrCmp $R0 $1 nyrva_contains_yes
  IntOp $4 $4 + 1
  Goto nyrva_contains_loop
nyrva_contains_yes:
  StrCpy $R0 "1"
  Goto nyrva_contains_done
nyrva_contains_no:
  StrCpy $R0 "0"
nyrva_contains_done:
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Exch $R0
FunctionEnd

; Remove only exact occurrences of $INSTDIR from a semicolon-delimited PATH.
Function un.NyrvaRemovePathEntry
  Exch $1
  Exch
  Exch $0
  Push $2
  Push $3
  Push $4
  StrCpy $2 ""
  StrCpy $3 "$0;"
nyrva_remove_loop:
  StrCmp $3 "" nyrva_remove_finish
  StrCpy $4 0
nyrva_find_sep:
  StrCpy $R0 $3 1 $4
  StrCmp $R0 ";" nyrva_have_part
  StrCmp $R0 "" nyrva_have_part
  IntOp $4 $4 + 1
  Goto nyrva_find_sep
nyrva_have_part:
  StrCpy $R0 $3 $4
  IntOp $4 $4 + 1
  StrCpy $3 $3 "" $4
  StrCmp $R0 "" nyrva_remove_loop
  StrCmp $R0 $1 nyrva_remove_loop
  StrCmp $2 "" 0 +3
    StrCpy $2 $R0
    Goto nyrva_remove_loop
  StrCpy $2 "$2;$R0"
  Goto nyrva_remove_loop
nyrva_remove_finish:
  Pop $4
  Pop $3
  Pop $1
  Pop $0
  Exch $2
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro NYRVA_ADD_USER_PATH
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro NYRVA_REMOVE_USER_PATH
!macroend
