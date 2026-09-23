# Nyrva Everywhere — terminal, integrações e API local

O `nyrva-telemetry` é o executável de console independente, distribuído junto com o aplicativo e o `nyrva-hook`. Não precisa abrir o desktop nem ter um display. O executável `nyrva` também encaminha os comandos abaixo ao mesmo núcleo antes de iniciar a interface gráfica. No Windows, prefira `nyrva-telemetry.exe` para scripts e terminais.

Os comandos leem observações locais. Não autenticam, renovam credenciais nem consultam provedores a cada prompt. Uma integração sem dados disponíveis continua sem dados: ausência não equivale a quota zero.

## Diretório e comandos

Por padrão os dados ficam no diretório de configuração do Nyrva: `%APPDATA%/nyrva` no Windows, `$XDG_CONFIG_HOME/nyrva` ou `~/.config/nyrva` no Linux. `NYRVA_DATA_DIR` substitui esse diretório. A opção global `--data-dir` vem **antes** do comando, aceita um caminho absoluto e tem precedência sobre a variável.

```sh
nyrva-telemetry status --json
nyrva-telemetry resets --json
nyrva-telemetry sessions --json
nyrva-telemetry cockpit --json
nyrva-telemetry projects --json
nyrva-telemetry agents --json
nyrva-telemetry history --provider antigravity --account active --json
nyrva-telemetry doctor --json
nyrva-telemetry export --json
nyrva-telemetry top
nyrva-telemetry top --once --json
nyrva-telemetry --data-dir /caminho/absoluto/dados status --json
```

`help` lista as opções de consulta. O JSON tem `schema_version: 1`. Contas, fontes, quotas e janelas permanecem independentes. `doctor` e `export` retornam diagnóstico sanitizado, não um dump do banco ou dos arquivos dos provedores.

Em um terminal interativo, `top` atualiza as observações em cache. Digite `s` + Enter para sessões, `p` + Enter para quotas e `q` + Enter para sair. Ctrl-C também encerra. O programa não muda os modos de entrada do terminal. `--interval` aceita 1–60 segundos; `--iterations`, 1–3600 atualizações. Sem terminal, `top` imprime apenas um quadro, sem ANSI, e encerra. `--no-ansi` desativa as sequências de tela também no terminal. A visualização textual exibe até 50 registros; os comandos JSON oferecem as visões completas dentro dos limites do núcleo.

## Statusline opt-in

São suportados os pontos documentados de Claude (`~/.claude/settings.json`) e Antigravity (`~/.gemini/antigravity-cli/settings.json`). Não é criado um slot fictício de statusline para Codex/Cursor. Suas observações existentes continuam consultáveis.

No Linux, indique o caminho absoluto do executável instalado:

```sh
nyrva-telemetry integrations plan claude --executable /usr/bin/nyrva-telemetry --json
nyrva-telemetry integrations install claude --executable /usr/bin/nyrva-telemetry --apply
nyrva-telemetry integrations remove claude --apply
```

No PowerShell, usando o executável na pasta de instalação:

```powershell
$cli = (Resolve-Path '.\nyrva-telemetry.exe').Path
& $cli integrations plan claude --executable $cli --json
& $cli integrations install claude --executable $cli --apply
& $cli integrations remove claude --apply
```

Substitua `claude` por `antigravity` para a outra integração. `--home` permite selecionar uma home de teste; `--account` define o identificador local da conta. Não troca a conta autenticada. Caminhos com expansão de shell perigosa são recusados, não executados. No Windows o comando usa PowerShell sem perfil e pode ser chamado pelo Git Bash ou PowerShell do provedor.

`plan` não escreve nada. Se existir uma `statusLine`, `install` exige também `--replace`; nunca executa nem encadeia o comando anterior automaticamente. Apenas o campo `statusLine` é gerenciado. Os demais campos são preservados semanticamente, embora a formatação JSON possa mudar. JSON inválido, chaves duplicadas e caminhos indiretos são recusados.

A remoção restaura somente o campo gerenciado. Se o usuário alterou a statusline depois da instalação, o Nyrva recusa sobrescrevê-la. Instalações repetidas são idempotentes. Recibos privados em `telemetry/integrations` permitem recuperação de interrupções; não devem ser copiados entre computadores nem incluídos em exports. A pasta e o executável escolhidos precisam continuar existindo: após mover a instalação, remova explicitamente a integração antiga e configure a nova.

## Preferências, privacidade e manutenção

```sh
nyrva-telemetry settings export --json
nyrva-telemetry settings import --apply < preferencias.json
nyrva-telemetry settings rollback --apply
nyrva-telemetry privacy status --json
nyrva-telemetry privacy clear --confirm
nyrva-telemetry alerts list --json
nyrva-telemetry alerts refresh --json
```

A exportação de preferências tem um envelope `{ "schema_version": 1, "settings": ... }`; a importação recebe **o objeto `settings`**, não o envelope. Por exemplo, no PowerShell:

```powershell
$settings = (& $cli settings export --json | ConvertFrom-Json).settings
$settings.retention_days = 30
$settings | ConvertTo-Json -Depth 12 | & $cli settings import --apply
```

Os controles de inclusão e metadados afetam novas gravações e visões do observatório, não desligam os coletores legados do notch. Desativar metadados não apaga registros anteriores. A limpeza remove histórico e alertas do Nyrva, preservando preferências e arquivos dos provedores; novas coletas podem gerar registros novamente.

```sh
nyrva-telemetry migrate status --json
nyrva-telemetry migrate --apply
nyrva-telemetry migrate rollback --apply
```

Migrações de banco são transacionais. O rollback da versão 2 para 1 só ocorre se for sem perda: colisões entre sessões independentes são recusadas, mantendo os dados e a versão originais. Versões futuras desconhecidas também são recusadas. Pare outros coletores durante manutenção; uma gravação posterior pode migrar novamente um banco antigo para a versão atual. Isso não é backup nem reversão de arquivos de provedores.

## API HTTP e eventos

A API só inicia por solicitação explícita:

```sh
nyrva-telemetry serve --port 0 --duration 3600
```

O processo atende em `127.0.0.1`; `--port 0` escolhe uma porta livre. Duração: 1–86400 segundos. Não fica residente após fechar o processo. Há apenas um servidor ativo por diretório de dados.

`telemetry/api-session.json` contém o endereço, o token aleatório de sessão e a expiração. O arquivo fica restrito ao usuário (permissões Unix ou DACL protegida no Windows). Não envie esse arquivo a terceiros nem coloque o token em URLs/logs. A saída normal do comando não imprime o token. Cada inicialização gera outro token. Encerramento normal remove o arquivo; uma interrupção forçada pode deixar um arquivo vencido, substituído na próxima inicialização. Não reutilize uma sessão vencida.

Clientes locais leem esse arquivo e enviam `Authorization: Bearer <token>` e `Host` exatamente igual ao endereço anunciado. A API recusa `Origin`, sites externos, Host divergente, autenticação ausente, cabeçalhos duplicados, corpos de requisição e qualquer método diferente de GET. Não habilita CORS. Não use um navegador web como cliente desta API.

Rotas de leitura: `/v1/status`, `/v1/cockpit`, `/v1/resets`, `/v1/sessions`, `/v1/projects`, `/v1/agents`, `/v1/doctor` e `/v1/history?provider=claude&account=active&source=statusline&limit=100`. Histórico aceita no máximo 200 observações por consulta. `/v1/events` entrega eventos SSE versionados e sanitizados; reconecte após o encerramento do stream, sempre respeitando a validade da sessão. Não transmite payload bruto nem token de acesso.

Limites: 8 clientes simultâneos, cabeçalhos de até 8 KiB, alvo de requisição de até 1024 bytes, respostas de até 2 MiB, prazo de leitura de cabeçalhos e timeout de escrita de 2 segundos. Cada stream envia no máximo 60 quadros e nunca ultrapassa a duração do servidor. Os limites são deliberados: esta API é uma interface local de observação, não um serviço multiusuário ou remoto.

## Verificação de atualização assinada

Instalação/download automático permanecem **desativados**. `update` implementa uma verificação local explícita, não o protocolo de atualização automática do plugin Tauri. Nenhuma chave de publicação real é criada ou escolhida silenciosamente.

```sh
nyrva-telemetry update status --json
nyrva-telemetry update configure --apply < confianca-do-publicador.json
nyrva-telemetry update verify --manifest release.json --signature release.sig --artifact pacote --json
```

Obtenha a chave pública por um canal confiável do publicador antes de configurar. A configuração aceita `schema_version: 1`, `public_key` (32 bytes Ed25519 em hexadecimal), `origin` (prefixo HTTPS terminado em `/`) e `minimum_version` (SemVer). Esses dados de confiança são privados e não integram a exportação de preferências.

O manifesto assinado contém exatamente `schema_version`, `version`, `target`, `url`, `size` e `sha256`. `target` usa `windows-x86_64` ou `linux-x86_64` para as distribuições atuais. A assinatura é Ed25519 sobre os **bytes exatos do manifesto**, armazenada em hexadecimal (64 bytes) no arquivo `.sig`. Reformatação JSON após assinar invalida a assinatura.

A verificação exige versão estável acima do mínimo configurado, sistema/arquitetura corretos, mesma origem e prefixo HTTPS, tamanho exato (até 512 MiB), assinatura válida e SHA-256 correspondente ao pacote. Credenciais em URL, query/fragment, assinatura errada e alteração do arquivo são recusadas. Um resultado válido não instala nem executa o arquivo. O mínimo configurado deve acompanhar a versão instalada; esta ferramenta não gerencia sozinho o ciclo de atualização nem comprova como um arquivo foi transportado pela rede.

## Evidência reproduzível e release

`cargo test --workspace --locked --no-fail-fast` mantém todos os contratos habilitados. `python scripts/smoke_everywhere.py --binary CAMINHO` executa fluxos de processos, arquivos, sockets e shell com home sintética isolada. Os testes Windows incluem Git Bash, PowerShell e cmd; Linux inclui pseudo-terminal real. As credenciais reais dos provedores não são necessárias.

A CI também constrói NSIS, Debian e AppImage, testa instalação isolada/extração e preserva evidências. O workflow de Experience cobre WebKit/WebView2 e SQLite reais separadamente. Esses testes automatizados não substituem a auditoria física de tray, monitores/DPI, leitor de tela e autenticação com as contas reais antes da publicação 1.0. Versões, tags e guardas de publicação não são alterados por este macrostep.

Referências primárias: https://code.claude.com/docs/en/statusline ; https://www.antigravity.google/docs/cli/statusline/ ; https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html .
