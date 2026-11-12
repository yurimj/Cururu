; Ganchos do instalador NSIS (requisitos 12 e 31).
;
; O instalador pergunta o idioma — ou o adivinha pelo idioma da máquina, que é
; o que o NSIS já faz sozinho ao pré-selecionar `$LANGUAGE` na lista. O que
; falta é o Cururu **saber** o que foi escolhido: sem isto, quem instalou em
; inglês abriria o aplicativo em português na primeira vez.
;
; A escolha vai para o registro do usuário, e o backend a lê na primeira
; execução (ver `idioma_do_instalador` em comandos.rs). HKCU, e não HKLM,
; porque a instalação é por usuário (`installMode: currentUser`) e escrever em
; HKLM exigiria Administrador — que este instalador não pede.
;
; Pelo mesmo HKCU o Cururu entra no "Abrir com" do `.sql` (requisito 12).

!macro NSIS_HOOK_POSTINSTALL
  ; `$LANGUAGE` guarda o LCID escolhido na tela de idioma. Português do Brasil
  ; e de Portugal caem os dois em pt-BR: é o único português que o Cururu tem,
  ; e é mais perto do que o inglês.
  ${If} $LANGUAGE == 1046
    WriteRegStr HKCU "Software\Cururu" "Idioma" "pt-BR"
  ${ElseIf} $LANGUAGE == 2070
    WriteRegStr HKCU "Software\Cururu" "Idioma" "pt-BR"
  ${ElseIf} $LANGUAGE == 1034
    WriteRegStr HKCU "Software\Cururu" "Idioma" "es"
  ${ElseIf} $LANGUAGE == 3082
    WriteRegStr HKCU "Software\Cururu" "Idioma" "es"
  ${Else}
    WriteRegStr HKCU "Software\Cururu" "Idioma" "en"
  ${EndIf}

  ; O Cururu se oferece para abrir .sql, e só isso: quem abre .sql por padrão
  ; continua sendo quem o usuário escolheu. Trocar é decisão dele, pelo
  ; "Abrir com" ou pelas Configurações do Windows — o instalador põe o Cururu
  ; na lista, e não toma o lugar do SSMS de ninguém sem perguntar.
  WriteRegStr HKCU "Software\Classes\Cururu.sql\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\.sql\OpenWithProgids" "Cururu.sql" ""
  ; Avisa o Explorer de que as associações mudaram (SHCNE_ASSOCCHANGED).
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Desinstalar não deixa rastro de configuração no registro.
  DeleteRegValue HKCU "Software\Cururu" "Idioma"
  DeleteRegKey /ifempty HKCU "Software\Cururu"

  ; O Cururu sai do "Abrir com". Numa atualização (`/UPDATE`) o instalador novo
  ; grava tudo de volta em seguida, e apagar no meio só arriscaria o Windows
  ; esquecer quem o usuário tinha escolhido.
  ${If} $UpdateMode <> 1
    DeleteRegKey HKCU "Software\Classes\Cururu.sql"
    ; Só o valor do Cururu: a chave é dividida com os outros programas que
    ; abrem .sql.
    DeleteRegValue HKCU "Software\Classes\.sql\OpenWithProgids" "Cururu.sql"
    ; Se ele tinha virado o padrão por aqui, o .sql volta para quem era antes.
    ReadRegStr $0 HKCU "Software\Classes\.sql" ""
    ${If} $0 == "Cururu.sql"
      DeleteRegValue HKCU "Software\Classes\.sql" ""
    ${EndIf}
    System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ${EndIf}
!macroend
