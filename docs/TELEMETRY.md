# Telemetria local — implementação em desenvolvimento da 1.0

Esta branch ainda **não é a release 1.0**. Windows + Linux X11 continuam sendo as plataformas de aceite. O novo core é independente de Tauri, mas compatibilidade do desktop exige a checklist manual existente.

## Comandos disponíveis

O executável desktop continua abrindo o notch quando chamado sem argumentos. Os comandos abaixo terminam antes de inicializar a interface gráfica:

```text
nyrva status
nyrva status antigravity --json
nyrva status --account work --json
nyrva resets --json
nyrva sessions --json
nyrva history antigravity --account active --limit 200 --json
nyrva forecast antigravity --bucket weekly --reserve 0.1 --json
nyrva statusline antigravity
nyrva ingest antigravity --account work --json
```

`nyrva-core` também fornece o binário opcional `nyrva-telemetry`, compilável com `cargo build -p nyrva-core --bin nyrva-telemetry`. Ele usa os mesmos comandos, sem dependências de janela. Os pacotes desktop não precisam de um segundo helper de telemetria: o próprio `nyrva` atende a statusline.

`status`, `resets` e `sessions` podem filtrar provider e alias explícito. O alias padrão de ingestão/histórico é `active`. Um alias é uma identificação local escolhida pelo usuário, **não autenticação nem detecção automática de conta**. O notch atual usa apenas `active`; outras contas aparecem nas consultas, sem alternância automática.

`history` conserva a fonte de cada observação. `forecast` exige ao menos três amostras comparáveis em cinco minutos, mesma conta/fonte/bucket/reset e leitura recente. A previsão retorna `null` sem base suficiente. Ela é uma estimativa, não garantia de tempo de trabalho. Não confundir essa implementação com toda a análise de sessões, custos ou agentes prevista para a 1.0.

## Antigravity CLI

A CLI expõe quotas à statusline documentada. Configure explicitamente o comando que recebe seu JSON, em vez de executar `/usage` em modo prompt.

Em `~/.gemini/antigravity-cli/settings.json`, **mescle** a propriedade abaixo com suas preferências existentes. Não substitua o arquivo todo:

```json
{
  "statusLine": {
    "type": "command",
    "command": "nyrva statusline antigravity"
  }
}
```

O executável precisa estar acessível no PATH da CLI. Caso não esteja, informe seu caminho absoluto, entre aspas quando houver espaços, conforme o shell utilizado pela CLI. O gerenciador automático/reversível de integrações ainda não foi entregue nesta etapa; preserve uma cópia de uma statusline personalizada antes de substituí-la. Restaurar a propriedade anterior remove a integração, sem tocar no login.

O Nyrva descarta campos desconhecidos e armazena apenas o contrato normalizado. Uma quota zerada é preservada; JSON inválido, payload acima de 256 KiB e alias inseguro são recusados. A entrada não executa comandos nem renova tokens. Apenas Antigravity tem ingestão de statusline implementada neste recorte; Claude/Codex não devem ser anunciados como integrados por esse mecanismo ainda.

Quando o desktop está aberto, uma leitura nova da CLI acorda o adapter do Antigravity. Uma resposta atrasada do IDE não sobrescreve uma leitura recente da CLI. Se a fonte deixa de informar a quota, o histórico conserva a leitura anterior como desatualizada. O fallback do IDE existente permanece disponível; login na CLI não é inferido a partir do login do IDE.

Documentação consultada: https://www.antigravity.google/docs/cli/statusline/

## Dados, privacidade e limitações

O banco fica em `<diretório de configuração>/nyrva/telemetry/history.sqlite3`. Em Windows, o diretório-base normalmente é `%APPDATA%`; no Linux, `$XDG_CONFIG_HOME` ou `~/.config`. `NYRVA_DATA_DIR` sobrescreve somente a raiz de telemetria, sendo útil para testes e execução headless. Ele não realoca as configurações legadas do desktop nem os arquivos dos providers.

Retenção: até 90 dias, com teto global de 50.000 observações; o teto pode reduzir o período disponível. No Unix, novos diretórios de telemetria e o banco são criados com permissões 0700/0600. Windows herda as permissões do diretório do usuário. SQLite migrações são versionadas; uma versão futura causa erro, não downgrade automático.

O novo histórico não armazena tokens de autenticação, e-mail, prompts, respostas ou caminhos absolutos de projetos. Pode conter alias, modelo, nome-base de projeto, identificador de sessão, branch e estado quando a fonte os fornece. Isso **não altera retroativamente** o comportamento de logs/sessões do aplicativo legado; a revisão ampla de privacidade faz parte dos próximos macrosteps.

A data de observação indica recebimento da fonte. Ela não comprova quando o provider consultou seu próprio backend. Reset passado não significa quota renovada: o Nyrva aguarda confirmação. Fontes permanecem separadas, sem soma de quotas ou atribuição automática de consumo a projetos concorrentes.

Não há API de rede nova, envio cloud, atualização automática, ingestão de conteúdo de prompts nem troca de contas. Dashboard novo, layouts, notificações, TUI, integração automática e aceite de release seguem pendentes no plano de três macrosteps.

## Verificação repetível

```bash
cargo test -p nyrva-core --locked
cargo build -p nyrva-core --bin nyrva-telemetry --locked
python scripts/smoke_telemetry.py --binary target/debug/nyrva-telemetry --output target/telemetry-smoke.json
```

No Windows, use `target/debug/nyrva-telemetry.exe`. O mesmo smoke aceita o executável desktop `nyrva`/`nyrva.exe`, após preparar o sidecar e compilar o workspace. O teste executa processos reais, usa somente fixtures sintéticas em uma pasta temporária e gera JSON verificável. Ele não prova o funcionamento visual do notch nem o recebimento do payload por uma instalação real da CLI do Antigravity.
