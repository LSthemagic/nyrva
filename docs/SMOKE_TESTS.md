# Aceite manual do Nyrva 1.0 — Windows e Linux X11

**Situação inicial: NÃO EXECUTADO. A primeira release pública está bloqueada.**

Este documento é o roteiro de validação, não evidência de aprovação. Um CI verde, um pacote gerado ou um PR mergeado não preenchem nenhuma célula automaticamente.

## Como registrar

Teste os três pacotes da mesma execução de CI/avaliação da release: NSIS no Windows, `.deb` e AppImage em Linux X11. Use pelo menos um ambiente Windows suportado e uma sessão Linux X11 real, registrando versões e limitações. Para afirmar suporte específico a outra distribuição, valide-a também.

Copie a ficha e a tabela para `docs/validation/<tag>-smoke.md` em um PR de evidências. Registre o commit completo testado, o link da execução, os nomes e SHA-256 dos pacotes. Se as evidências forem commitadas depois do build, mantenha o SHA do **build testado**, não o SHA do commit de documentação.

Resultados permitidos: **PASSOU**, **FALHOU**, **BLOQUEADO**, **NÃO EXECUTADO**. Só use **NÃO APLICÁVEL** com justificativa aprovada; falta de uma conta/provedor é BLOQUEADO, não aprovação. Não anexe segredos, transcrições privadas nem bancos dos provedores.

## Ficha por ambiente

| Campo | Registro inicial |
|---|---|
| Responsável e data/hora | NÃO EXECUTADO |
| Tag, commit completo e execução de origem | NÃO EXECUTADO |
| Nome e SHA-256 de cada pacote | NÃO EXECUTADO |
| Sistema operacional, versão e arquitetura | NÃO EXECUTADO |
| Desktop/gerenciador de janelas e tipo de sessão Linux | NÃO EXECUTADO |
| Monitores, resolução e escala/DPI | NÃO EXECUTADO |
| Versões dos provedores e condições das contas, sem segredos | NÃO EXECUTADO |
| Evidências sanitizadas e defeitos encontrados | NÃO EXECUTADO |

## Casos obrigatórios

Execute com usuário comum, a partir dos arquivos instalados. Testes que alteram hook/autostart devem ser reversíveis e preservar configurações preexistentes.

| ID | Ação e resultado esperado | Windows NSIS | Linux deb | Linux AppImage |
|---|---|---|---|---|
| S01 | Instalar/executar o pacote; a janela aparece sem crash nem erro de biblioteca ausente. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S02 | Conferir borda da tela e escala; arrastar, fechar e reabrir; posição persistida e acessível. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S03 | Interagir com bandeja, menu e janela sem roubo indevido de foco; links abrem o destino correto. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S04 | Ativar início automático, terminar/iniciar sessão e conferir execução; desativar e repetir. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S05 | Instalar hook Claude, executar uma sessão e conferir atividade; fechar Nyrva e verificar relançamento; remover hook preservando outras configurações. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S06 | Codex autenticado: conferir uso/atividade com raiz padrão e `CODEX_HOME` alternativo, sem alteração de credenciais. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S07 | Cursor autenticado: conferir uso/atividade com IDE aberta e fechada, leitura do banco sem escrita nem bloqueio provocado pela Nyrva. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S08 | Antigravity aberto: conferir conexão local; fechado: conferir dados antigos/alternativa sem inventar quota atual; testar ausência/bloqueio do chaveiro quando aplicável. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S09 | Provedor ausente/deslogado e rede indisponível: erro isolado; demais provedores e interface continuam utilizáveis. Não apague credenciais para simular a falha. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S10 | Revisar diagnósticos, logs e estado persistido gerados no teste: nenhum token, segredo CSRF ou conteúdo de credenciais exposto. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S11 | Fechar e reabrir repetidamente; sem processos inesperados ou perda de configuração. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S12 | Remover hook/autostart antes de desinstalar ou remover o AppImage; sem referência quebrada e sem remover dados de autenticação dos provedores. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |

## Experience e Everywhere — complemento do aceite físico

O responsável pelo projeto executa estes casos localmente. Eles permanecem **NÃO EXECUTADOS** até o registro real; o aceite automatizado não preenche esta tabela. Não execute importação, limpeza ou migração sobre seu estado habitual para experimentar: use um `--data-dir` absoluto e uma `--home` sintética conforme o guia Everywhere.

| ID | Ação e resultado esperado | Windows NSIS | Linux deb | Linux AppImage |
|---|---|---|---|---|
| S13 | Abrir/reabrir cockpit pela bandeja sem deadlock ou janelas duplicadas; alternar quotas, sessões, projetos e agentes mantendo o notch. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S14 | Navegar por teclado/leitor de tela, conferir foco, zoom/escala e contraste nos ambientes realmente disponíveis. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S15 | Abrir `nyrva-telemetry top`, alternar `s`/`p` + Enter e sair com `q` + Enter/Ctrl-C; terminal normal depois da saída. Conferir `status --json` sem abrir GUI. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S16 | Executar `integrations plan`; após confirmação, instalar statusline em home sintética e removê-la. Preservar campos anteriores; alteração manual posterior deve impedir sobrescrita. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S17 | Iniciar API explícita com duração curta e diretório sintético; autenticação obrigatória, arquivo de sessão privado, sem token em saída/log e remoção após saída normal. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S18 | Em dados sintéticos, exportar/importar somente o objeto `settings`, alterar preferência e fazer rollback. Export não inclui prompts, credenciais nem recibos de integração. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S19 | Conferir `update status` sem confiança configurada: instalação automática desativada e verificação recusada. Não configure uma chave arbitrária para aprovar o teste. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |
| S20 | Conferir os três executáveis instalados, executar o smoke isolado e remover integrações antes de mover/desinstalar. No AppImage, conferir também relançamento e caminho estável. | NÃO EXECUTADO | NÃO EXECUTADO | NÃO EXECUTADO |

O smoke isolado é reproduzível com `python scripts/smoke_everywhere.py --binary CAMINHO_ABSOLUTO --output evidencia.json`; rode para o console e para o desktop, usando os arquivos do pacote sob teste. Consulte [Everywhere](everywhere.md) para comandos, formato de preferências e limites. Autenticação real e integração visual com cada provedor precisam do aceite do responsável, sem publicar arquivos de autenticação ou `api-session.json`.

Em S02, teste múltiplos monitores quando disponíveis e registre a cobertura real; não marque como testada uma configuração que não foi usada. Em S08, compare com o estado real do provedor e registre qual fonte foi utilizada.

## Defeitos e reteste

Cada falha deve registrar ID do caso, ambiente, passos, resultado esperado/observado e evidência sanitizada. Corrija em PR separado ou no PR em revisão, gere novos pacotes e repita os casos afetados e a regressão necessária. Não reutilize o aceite de um binário antigo para aprovar um novo.

## Decisão de publicação

- [ ] CI e empacotamento do commit/tag aprovados; os três pacotes e seus hashes estão identificados.
- [ ] Casos obrigatórios executados e aprovados no Windows e Linux X11, sem bloqueios de provedor ocultos.
- [ ] Nenhuma falha bloqueadora aberta; limitações restantes foram documentadas e aceitas explicitamente.
- [ ] PR de evidências revisado por responsável identificado.
- [ ] Responsável conferiu se os arquivos anexados à release são os mesmos arquivos testados.
- [ ] Publicação autorizada com responsável e data registrados.

**Decisão atual: NÃO AUTORIZADA — falta execução manual.** A autorização futura deve constar da ficha da versão testada; não altere este modelo para fingir que uma execução ocorreu.
