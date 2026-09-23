# Guia do Nyrva 1.0

Guia de uso para **Windows** e **Linux X11**. Para compilar ou publicar o projeto, use a documentação de desenvolvimento/release.

## 1. O que a Nyrva faz

A Nyrva reúne informações locais dos seus assistentes de programação com IA em três superfícies:

1. **Desktop** — notch compacto + observatório com quotas, resets, sessões, projetos, agentes e alertas.
2. **Terminal** — `nyrva-telemetry` para dashboard e consultas JSON.
3. **Integrações locais** — statusline opt-in e API HTTP/SSE local, autenticada e somente leitura.

Na 1.0, os provedores são **Claude Code, Codex, Cursor e Antigravity**. A Nyrva não faz login por você. Autentique normalmente no aplicativo/CLI de cada provedor.

## 2. Instalação

Baixe em [Releases](https://github.com/LSthemagic/nyrva/releases/tag/v1.0.0).

### Windows

Baixe `Nyrva_1.0.2_x64-setup.exe` e execute o instalador. O pacote contém o desktop `nyrva.exe`, a CLI `nyrva-telemetry.exe` e o helper `nyrva-hook.exe`.

Os pacotes 1.0 ainda não possuem assinatura de publicador. Não desative proteções do Windows; confira a origem e o hash em `SHA256SUMS`.

### Debian / Ubuntu em X11

```bash
sudo apt install ./Nyrva_1.0.2_amd64.deb
```

Execute como usuário normal, não como root.

### AppImage

```bash
chmod +x ./Nyrva_1.0.0_amd64.AppImage
./Nyrva_1.0.0_amd64.AppImage
```

Mantenha o AppImage em um caminho estável se usar integrações que precisem relançar a aplicação. Wayland não faz parte do alvo da 1.0.

## 3. Primeiro uso

1. Abra e autentique os provedores que utiliza.
2. Inicie a Nyrva.
3. Use o notch para acompanhar o estado essencial.
4. Abra o observatório para quotas, resets, sessões, projetos e agentes.
5. Informação ausente deve aparecer como indisponível/stale — ausência não significa quota 0%.

Codex respeita `CODEX_HOME` ou `~/.codex`; Cursor usa seu banco local; Antigravity prefere o bridge local e possui fallbacks; Claude pode usar a integração/hook da Nyrva.

## 4. Terminal

A CLI funciona sem abrir a GUI:

```bash
nyrva-telemetry status --json
nyrva-telemetry resets --json
nyrva-telemetry sessions --json
nyrva-telemetry cockpit --json
nyrva-telemetry projects --json
nyrva-telemetry agents --json
nyrva-telemetry history antigravity --account active --json
nyrva-telemetry doctor --json
nyrva-telemetry export --json
```

Dashboard interativo:

```bash
nyrva-telemetry top
```

Controles: `s` + Enter para sessões, `p` + Enter para quotas, `q` + Enter ou Ctrl-C para sair.

Snapshot não interativo:

```bash
nyrva-telemetry top --once --json
```

As saídas JSON públicas têm `schema_version: 1`.

## 5. Dados e testes isolados

Padrões: `%APPDATA%/nyrva` no Windows; `$XDG_CONFIG_HOME/nyrva` ou `~/.config/nyrva` no Linux.

```bash
nyrva-telemetry --data-dir /caminho/absoluto/teste status --json
```

Também existe `NYRVA_DATA_DIR`; `--data-dir` tem precedência. Não experimente comandos destrutivos no seu diretório habitual.

## 6. Statusline do Claude e Antigravity

A instalação é **opt-in**. Comece com `plan`, que não altera arquivos.

Linux:

```bash
nyrva-telemetry integrations plan claude --executable /usr/bin/nyrva-telemetry --json
nyrva-telemetry integrations install claude --executable /usr/bin/nyrva-telemetry --apply
nyrva-telemetry integrations remove claude --apply
```

Windows PowerShell:

```powershell
$cli = (Resolve-Path '.\nyrva-telemetry.exe').Path
& $cli integrations plan claude --executable $cli --json
& $cli integrations install claude --executable $cli --apply
& $cli integrations remove claude --apply
```

Troque `claude` por `antigravity`. Se já existir uma statusline, a Nyrva exige `--replace`. Ela preserva campos alheios; se você alterar a statusline depois, a Nyrva se recusa a sobrescrever sua mudança silenciosamente.

### Codex

O Codex usa a própria statusline nativa. A Nyrva adiciona os indicadores nativos `five-hour-limit` e `weekly-limit` sem substituir os outros itens escolhidos pelo usuário:

```powershell
nyrva-telemetry integrations plan codex
nyrva-telemetry integrations install codex --apply
nyrva-telemetry integrations remove codex --apply
```

A alteração é feita pela API oficial `codex app-server`, com controle de versão do arquivo e verificação da configuração efetiva. `remove` remove somente os dois indicadores gerenciados pela Nyrva. Reabra o Codex para ver a linha atualizada.

## 7. Preferências, privacidade e alertas

```bash
nyrva-telemetry settings export --json
nyrva-telemetry settings rollback --apply
nyrva-telemetry privacy status --json
nyrva-telemetry alerts list --json
nyrva-telemetry alerts refresh --json
nyrva-telemetry privacy clear --confirm
```

`privacy clear` limpa dados pertencentes à Nyrva, não credenciais/arquivos dos provedores. A importação recebe o objeto `settings` exportado e há rollback da versão anterior.

## 8. Migrações

```bash
nyrva-telemetry migrate status --json
nyrva-telemetry migrate --apply
nyrva-telemetry migrate rollback --apply
```

As migrações são transacionais. Rollback que causaria perda ou colisão é recusado.

## 9. API local e SSE

```bash
nyrva-telemetry serve --port 0 --duration 3600
```

A API escuta somente em `127.0.0.1`, gera token por sessão, exige `Authorization: Bearer <token>`, é somente leitura e não habilita CORS. O endereço/token temporário fica em `telemetry/api-session.json`. **Não compartilhe esse arquivo.**

Rotas:

```text
/v1/status
/v1/cockpit
/v1/resets
/v1/sessions
/v1/projects
/v1/agents
/v1/doctor
/v1/history?provider=claude&account=active&source=statusline&limit=100
/v1/events
```

`/v1/events` fornece SSE sanitizado. A API é para automações locais, não exposição na rede.

## 10. Diagnóstico

```bash
nyrva-telemetry doctor --json
nyrva-telemetry status --json
nyrva-telemetry privacy status --json
```

Se um provedor não aparecer, confirme que está instalado/autenticado e possui estado local. Não publique `auth.json`, bancos dos provedores, tokens, prompts privados ou `api-session.json`.

## 11. Atualizações e limitações

`nyrva-telemetry update status --json` mostra o estado de confiança/verificação. Na 1.0, `update` verifica artefatos quando há confiança Ed25519 explícita; **não instala atualizações automaticamente**.

Limitações atuais: Windows x64 e Linux x86_64/X11; sem suporte oficial a Wayland/macOS; pacotes sem assinatura de publicador; formatos locais dos provedores podem mudar.

Para a referência técnica completa, consulte [Everywhere](everywhere.md).
