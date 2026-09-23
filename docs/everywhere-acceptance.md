# Macrostep 3 — Everywhere: implementação e aceite

## Escopo e identidade

Windows e Linux X11 são as plataformas desta entrega. O mantenedor assumiu os testes físicos na própria máquina; nenhum resultado manual foi inventado. Wayland, macOS, novos provedores e instalação automática de atualizações continuam fora do escopo aprovado.

O checkpoint recuperado foi `6789699464f5bf76b4b1c0773e4617b38e518164`, com seus 15 commits anteriores preservados. O último commit de alteração executável é `7c2a6b301302e1a525825043e0d37feef8d3da76`; as mudanças posteriores desta conclusão são documentais. Não houve reescrita de histórico, alteração da main, tag, chave de assinatura nem publicação.

A execução de referência do código é [Nyrva Everywhere — 35881107675](https://github.com/LSthemagic/nyrva/actions/runs/35881107675). A CI também executa o checkout de cada revisão documental. **O aceite automatizado exige todos os 11 jobs verdes e os relatórios dos pacotes instalados; a existência deste documento, um build isolado ou um teste parcial não substituem esse resultado.** Confira o SHA em `everywhere-source/commit.txt` e use os pacotes da mesma execução. A confirmação final e os links da execução ficam no PR de entrega.

## Cobertura da implementação

| Entrega | Implementação e critério de aceite |
|---|---|
| Terminal e entrypoints | Desktop e `nyrva-telemetry` compartilham o core; JSON versionado, leitura sem criar histórico/GUI, `top` redirecionado e em pseudo-terminal, saída por `q` + Enter. |
| Integrações reversíveis | Plano sem mutação; instalação/substituição explícitas; execução real da statusline; preservação de campos alheios; recusa de alterações posteriores, JSON duplicado e caminhos inseguros; recuperação de remoção interrompida. |
| API e eventos | Loopback, token aleatório privado, um servidor por diretório, autenticação, Host/Origin, método, tamanho e conexões limitados; SSE sanitizado, rotação/expiração/limpeza; requisições TCP atrasadas e fragmentadas. |
| Preferências e manutenção | Importação validada do objeto `settings`, rollback, limpeza somente de dados Nyrva, migração transacional v1/v2 sem perda; versões futuras e colisões recusadas. |
| Atualizações | Confiança explícita e verificação local Ed25519, origem, versão, target, tamanho e SHA-256; adulteração e ausência de confiança recusadas. Não baixa, instala ou executa o pacote. |
| Distribuição | NSIS, Debian e AppImage com desktop, console, hook e avisos de licença; smoke dos executáveis reais instalados/extraídos; regressão Core/Experience preservada. |

Os contratos detalhados estão em `nyrva-core/tests/everywhere_extended.rs`, `api_transport.rs`, `windows_statusline.rs`, `nyrva/tests/everywhere_entrypoints.rs`, `tests/test_everywhere_http.py` e `scripts/smoke_everywhere.py`. Não existem providers simulados contabilizados como autenticação real.

## Correções da retomada e evidência

1. `4a5543df`: o cliente de teste passou a encerrar a leitura no `Content-Length`, não no fechamento TCP. A regressão de socket primeiro falhou; leitura truncada, ambígua e excessiva continua rejeitada. Isso isoladamente não resolveu o defeito do servidor.
2. `64919eb6` e `712c9461`: uma regressão real de subprocesso/TCP reproduziu no Windows resposta antes dos cabeçalhos completos. O servidor agora torna cada socket aceito bloqueante antes do I/O com timeout, mantendo listener não bloqueante, autenticação e limites. RED: execução `35876635274`, artefato `10759000389`. GREEN do core: execução `35877368334`, 70 testes Windows e 69 Linux, sem falhas, ignorados ou filtros.
3. `7c2a6b30`: o smoke iniciado por PowerShell 7 herdava módulos incompatíveis no processo Windows PowerShell 5.1 de leitura de ACL. A chamada filha agora reconstrói `PSModulePath`; não relaxa as permissões nem o timeout. Foram adicionados contratos de isolamento de ambiente e smoke real iniciado por PowerShell. O core Windows e esse novo smoke passaram na execução de referência. Os testes Python locais passaram 38/38.

A regressão de apresentação passou 10/10. Na execução anterior, a regressão nativa Experience passou em Windows e X11; a distribuição Linux passou em instalação Debian e AppImage. O console extraído do novo Debian e o **AppImage externo**, com `APPIMAGE_EXTRACT_AND_RUN=1`, também passaram localmente pelos sete grupos de smoke. A falha da execução anterior no smoke Windows instalado não foi escondida: exigiu a terceira correção e nova execução completa.

Os artefatos de cada CI separam `everywhere-core-Windows`, `everywhere-core-Linux`, `everywhere-distribution-Windows`, `everywhere-distribution-Linux`, evidências `experience-completion-*` e pacotes `nyrva-windows`/`nyrva-linux`. Os pacotes continuam com a versão existente **0.3.0**: não foram renomeados para simular uma publicação 1.0.

## Reproduzir sem tocar nos dados habituais

Use o script do checkout e os executáveis do pacote do mesmo SHA:

```powershell
python scripts/smoke_everywhere.py --binary 'C:\caminho\instalado\nyrva-telemetry.exe' --output evidencia-cli.json
python scripts/smoke_everywhere.py --binary 'C:\caminho\instalado\nyrva.exe' --output evidencia-desktop.json
```

```sh
python3 scripts/smoke_everywhere.py --binary /usr/bin/nyrva-telemetry --output evidencia-cli.json
python3 scripts/smoke_everywhere.py --binary /usr/bin/nyrva --output evidencia-desktop.json
```

O smoke cria home e dados sintéticos, executa os fluxos reais e os remove ao terminar. Não exige credenciais dos provedores. Fora desse script, use `--data-dir` absoluto **antes do comando** e `--home` sintética nas integrações. Não teste limpeza ou migração no histórico habitual. No AppImage, confira o lançamento externo e não instale uma statusline apontando para uma montagem temporária. Veja o [guia Everywhere](everywhere.md).

## Aceite físico e publicação separados

[SMOKE_TESTS.md](SMOKE_TESTS.md), incluindo S13–S20, continua **NÃO EXECUTADO** até o registro do mantenedor. Autenticação real, bandeja, monitores/DPI, leitor de tela e particularidades do desktop pessoal não são comprovados por um runner de CI.

A revisão desta sessão foi de autor, com regressões executáveis; não houve reviewer independente. Warnings legados não são apresentados como resolvidos. O fechamento da implementação não autoriza merge, troca da versão, assinatura ou publicação. Esses passos dependem do mantenedor conforme [RELEASING.md](RELEASING.md); nenhum bloqueio de publicação foi removido.
