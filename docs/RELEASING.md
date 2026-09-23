# Preparação e publicação de uma versão

## O que a automação faz

`.github/workflows/release.yml` aceita uma tag `v*` ou uma alteração revisada em `.github/release-request.json` integrada à `main`. Também permite reexecução manual selecionando uma tag existente. Um disparo manual sobre uma branch é recusado.

Nos dois caminhos, o build usa o SHA exato do evento e chama o próprio `ci.yml` por `workflow_call`; não troca para uma `main` mais recente. Os testes das ferramentas de release, Windows e Linux precisam passar. O CI verifica `nyrva-hook`, `nyrva-telemetry`, `LICENSE`, `CREDITS.md` e `PROVIDER_GLYPH_NOTICES.md` dentro dos pacotes.

Depois do build, o fluxo exige exatamente um `.exe`, um `.deb` e um `.AppImage`, rejeita arquivos vazios, duplicados ou com nomes inseguros e gera `SHA256SUMS`. As cópias legíveis dos três avisos também são anexadas e entram no manifesto de hashes.

Somente o job final tem `contents: write`. Ele cria um **rascunho de GitHub Release**, nunca uma publicação estável automática. Não existem tokens pessoais nem credenciais de assinatura fictícias. PRs normais não criam tags ou releases.

## Escolher a versão

O mantenedor deve revisar e integrar o PR com CI verde. A versão deve coincidir em `nyrva/Cargo.toml` e `nyrva/tauri.conf.json`; a tag é `v` seguida dessa versão exata. Uma mudança de versão exige também a atualização correspondente do `Cargo.lock`.

Mantenha as notas com créditos e limitações em `docs/releases/<tag>.md`. O fluxo recusa notas ausentes ou vazias. Não crie tags falsas de teste, não reaproveite uma tag publicada e não declare testes manuais que não foram executados.

Na raiz, com Python 3.11+:

```bash
python -m unittest discover -s tests -p 'test_*.py' -v
python scripts/release_tools.py validate-tag --ref-type tag --tag v0.3.0
```

Esses comandos validam arquivos locais; não escrevem no GitHub.

## Caminho A — solicitar o candidato por PR

Atualize deliberadamente `.github/release-request.json` para a versão escolhida, junto das versões e das notas. O arquivo aceita somente esta estrutura:

```json
{
  "tag": "v0.3.0"
}
```

Ao integrar a alteração na `main`, o workflow **Release** valida o pedido e compila/testa/empacota esse commit. Só depois dos jobs aprovados ele cria a tag no SHA exato do build e anexa os arquivos ao rascunho. Não altera ou substitui uma tag existente que aponte para outro commit.

O `GITHUB_TOKEN` usado para criar a tag não dispara outro workflow de push. Por isso a mesma execução já realiza o build e cria o rascunho; esse caminho não depende de um segundo disparo implícito. O caminho por tag continua disponível normalmente.

Alterar apenas README, notas antigas ou outro arquivo não solicita uma release: o disparo de branch exige mudança em `.github/release-request.json`. Para uma versão futura, atualize o pedido em um novo PR. Não altere o pedido só para repetir uma release já existente.

## Caminho B — criar uma tag explicitamente

Após escolher um commit limpo, revisado e com CI aprovado, confira seu SHA e crie a tag. Para a versão `0.3.0`, os comandos abaixo são ações reais:

```bash
git status --short
git rev-parse HEAD
git tag -a v0.3.0 -m "Nyrva 0.3.0 release candidate"
git push origin v0.3.0
```

Execute apenas no commit escolhido e quando a tag ainda não existir. Não crie uma segunda tag se o caminho A já a criou. O workflow valida que a tag continua apontando para o commit compilado antes de anexar os arquivos.

## Aceite antes da publicação

Acompanhe o workflow **Release** e confira os três pacotes, os avisos e `SHA256SUMS` no rascunho. A existência da tag ou do rascunho não aprova a release 1.0.

Baixe os arquivos com uma conta que tenha acesso e execute [SMOKE_TESTS.md](SMOKE_TESTS.md). Registre tag, SHA do build, execução, hashes, ambientes, resultados e responsável em um PR de evidências. Os arquivos publicados devem ser exatamente os testados.

Com todos os arquivos e o manifesto na mesma pasta, no Linux:

```bash
sha256sum -c SHA256SUMS
```

No PowerShell, calcule o hash do instalador e compare com a entrada exata do manifesto:

```powershell
Get-FileHash ./NOME_DO_INSTALADOR.exe -Algorithm SHA256
```

Os pacotes não são assinados. SHA-256 verifica integridade, não identidade do publicador. Não desative proteções do sistema operacional para instalar o candidato.

Somente depois do aceite e da autorização do responsável, publique o rascunho pela interface do GitHub ou com o GitHub CLI autenticado:

```bash
gh release edit v0.3.0 --draft=false
```

Use a versão realmente testada. Um eventual pré-lançamento público precisa ser anunciado explicitamente como tal, com suas limitações; não é uma versão estável validada.

## Falhas e recuperação

Se versão, testes ou pacotes falharem, corrija antes da publicação. Não misture artefatos de commits ou execuções diferentes.

O fluxo nunca move tags nem sobrescreve releases. Se a tag já foi criada no mesmo SHA, uma reexecução pode reutilizá-la; se apontar para outro SHA, a operação falha. Se houver um rascunho incompleto, revise-o antes de agir: um mantenedor pode remover somente esse rascunho, preservando a tag, e reexecutar o workflow original ou selecionar essa tag em **Run workflow**.

Nunca apague uma release publicada ou sua tag para reaproveitar o número. Mudanças no código depois da tag exigem uma nova versão, novos pacotes e novo aceite.

## Apresentação do repositório

O About é metadado administrativo do GitHub, não é atualizado por um commit no README. Descrição sugerida:

> Cross-platform AI coding usage monitor for Windows and Linux X11, built with Rust + Tauri. Independent fork of Codenotch.

Mantenha os créditos explícitos no README, em `CREDITS.md` e nas notas. Renomear o repositório é uma operação separada; os workflows usam `github.repository`, não um endereço futuro fixo.

## Referências

- [CI reutilizável](../.github/workflows/ci.yml)
- [GitHub: disparos e GITHUB_TOKEN](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)
- [GitHub CLI: criar releases](https://cli.github.com/manual/gh_release_create)
- [Tauri: incluir recursos nos pacotes](https://v2.tauri.app/develop/resources/)
- [Especificação de aceite](superpowers/specs/2026-09-09-linux-provider-parity-and-release-design.md)
