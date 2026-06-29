# Fase 2 — Biblioteca e metadados

> Enriquece o produto: deixa de ser só "ligar/desligar o share" e passa a
> **entender o catálogo** (identificar jogos, nomear art, contar/medir) e a
> **consumir capas das fontes externas**. Constrói sobre o `core` da Fase 1
> (`catalog`/`meta`) sem tocar a regra de inversão de dependência.

- **Status:** Concluída (pendente PR para `main`) — todos os 4 critérios de
  aceitação fechados e **validados em campo** com PS2 + OPL v1.2.0-beta-2012
  (2026-06-28). Achados de campo viram backlog pós-fase (Fase 3), não bloqueiam o
  fechamento. Residual: um `stat` de 0644 que o usuário roda na máquina.
- **Criado em:** 2026-06-27
- **Última atualização:** 2026-06-28 (validação em campo PS2+OPL: guest, auth
  real, `reload` e capas OK → fase pronta para PR; fixes de UX: filtro de
  extensão no scan + senha numa linha nas Configurações. core 32 / infra 37
  testes verdes; CI verde no `origin`)

## Contexto e objetivo
O OPL descobre jogos pela estrutura de pastas e identifica cada um pelo **Game
ID** (ex.: `SLUS_213.86`), lido do `SYSTEM.CNF` da ISO. Para exibir um catálogo
rico e baixar a capa certa, precisamos extrair esse ID de forma confiável e casá-lo
com os art databases prontos (§7). O resultado: a UI lista os jogos com
título/ID/mídia/tamanho e o app baixa as capas por Game ID, gravando em `ART/`
com a nomenclatura do OPL. Inclui também autenticação opcional usuário/senha no
share.

## Escopo
- **Dentro:** extração do Game ID via `SYSTEM.CNF` (ISO9660); catálogo
  enriquecido (Game ID + título no `meta`); `ArtProvider` que baixa capa por Game
  ID das fontes externas e grava em `ART/`; UI com listagem rica
  (título/ID/mídia/tamanho/contagem); autenticação opcional (usuário/senha) no
  share, além do guest.
- **Fora:** UDPBD, tray, i18n externo, FTP (Fase 3); **web scraping de capas**
  (proibido §7 — só fontes consumíveis); thumbnails/cache de imagem sofisticados;
  edição de metadados pela UI (só leitura/derivação nesta fase).

## Decisões
| Data | Decisão | Justificativa | Alternativas consideradas |
|------|---------|---------------|---------------------------|
| 2026-06-27 | Game ID lido do `SYSTEM.CNF` (`BOOT2`) de dentro da ISO | Robusto e fiel ao PyOPLM; independe de o usuário ter renomeado a ISO | Extrair do nome do arquivo (quebra fora da convenção); ambos com fallback |
| 2026-06-27 | Art **baixado sob demanda** por Game ID das fontes externas (Kira/danielb) | Entrega o valor central (capa automática) sem o usuário caçar art | Apontar pasta local de art; híbrido local+download |
| 2026-06-27 | Parse ISO9660 (PVD, registros de diretório) como funções PURAS no `core`; só o seek/read fica na `infra` | Mantém a regra de inversão: parsers testáveis sem disco; `infra` só alimenta bytes | Reader monolítico na `infra` (parsers não testáveis isoladamente) |
| 2026-06-27 | HTTP via `ureq` 3.3.0 (bloqueante, rustls) | Sem runtime async; casa com a worker thread; TLS estático simplifica o `.deb` | `reqwest` (async, arrasta tokio); `curl` via shell (dep externa) |
| 2026-06-27 | Fonte de art = DB OPLM (danielb) no archive.org, baixando arquivo de dentro do zip por Game ID; **base URL configurável** com default no item atual | É a única fonte com extração por arquivo (Kira é `.7z` não extraível); configurável driblando o 503/mirrors | Baixar o zip/7z inteiro (6 GB, inviável por jogo); só pasta local (perde o auto-download) |
| 2026-06-27 | Auth opcional via `valid users = <user>` + `guest ok = no`; usuário = **dono da pasta** (conta já existente), senha definida por `smbpasswd -s -a` (stdin, nunca argv); guest segue padrão | Não criar contas de sistema (sem `useradd`: menor footprint e risco); reaproveita a conta que já é `force user` | Criar usuário Samba dedicado (exige `useradd`, gestão de conta); senha em argv (vazaria no `ps`) |
| 2026-06-27 | Senha transitória: fora do `ShareConfig` (que é `Eq`/`Debug`), só em `SmbBackend.auth_password` com `Debug` redigido | Evita vazar a senha em logs/serialização/comparações | Pôr a senha no `ShareConfig` (vazaria em `Debug`/logs) |
| 2026-06-28 | Modelo "aplicar/remover config + toggle único": UI com um só botão (Ativar/Desativar), `status` = config do OPL aplicada (`opl_share.conf`+include), `reload` no lugar de `restart` | Não parar/reiniciar o `smbd` global quebra outros usos do Samba e viola o isolamento (§0); um só botão é coerente com o status real | Manter os dois botões + start/stop global (conflitante e invasivo) |
| 2026-06-28 | **Trait `StorageBackend` sem `start`/`stop`** (só `apply_config`/`status`/`rollback`) — **diverge da lista de §3 do CLAUDE.md de propósito** | `start`/`stop` faziam `systemctl start/stop smbd` global (anti-padrão); controle de ciclo de vida de processo só faz sentido no futuro `UdpbdBackend`, quando a abstração será revisitada com os dois casos concretos (§7.1) | Manter `start`/`stop` (código morto que viola o isolamento) |
| 2026-06-28 | Painel de Configurações separado (gear no topo) movendo "Acesso ao share" para fora da tela principal | Opções inline empurravam o catálogo para fora da área visível; separar configuração de operação (feedback de uso real) | Manter o bloco inline (cresce a tela, compete com o catálogo) |

## A validar no ambiente
- [x] **Crate HTTP:** `ureq` **3.3.0** (mar/2026) — síncrono/bloqueante, TLS via
      `rustls` por padrão, **sem runtime async**. Casa com a worker thread já
      usada na UI. (Confirmar download para o cache offline ao adicionar a dep.)
- [x] **Fonte e endpoints (confirmados via PyOPLM `storage.py` + archive.org):**
      DB mensal do OPL Manager (danielb). Item canônico atual: `OPLM_ART_2023_11`
      (zip 6.3 GB, 96.155 arquivos). Estrutura interna:
      `PS2/<GameID>/<GameID>_<TIPO>.<ext>` (ex.: `PS2/SCUS_973.13/SCUS_973.13_COV.jpg`).
      URL por arquivo: `https://archive.org/download/<ITEM>/<ITEM>.zip/PS2/<id>/<id>_COV.jpg`
      (tentar `.jpg` e `.png`). O set do Kira é um `.7z` único (não extraível por
      arquivo no archive.org) → fica como fonte secundária/offline.
- [x] **Sufixos de ART do OPL (do PyOPLM):** `COV` (capa frente), `COV2` (verso),
      `ICO`, `LAB` (rótulo do disco), `LGO` (logo), `SCR`/`SCR2` (screenshots),
      `BG` (fundo). BG/SCR têm variantes numeradas `_NN` na fonte. Destino em
      `ART/`: `<GameID>_<TIPO>.<ext>`. V1 prioriza `COV`.
- [x] Leitor ISO9660: escrito mínimo (PVD + raiz + `SYSTEM.CNF`) e testado com ISO
      sintética. **Validado com ISOs reais** (backup `OPL_BACKUP` do usuário): Game
      ID extraído e catálogo rico exibido corretamente na UI.
- [x] Mecanismo de auth do Samba para o share (`smbpasswd -a`, `guest ok = no`,
      `valid users`) — **conf autenticado validado com `testparm`** (Samba 4.23.x,
      "Loaded services file OK"; só os avisos esperados de NT1/lanman, §0). Falta
      a confirmação de conexão fim-a-fim com um cliente SMB real.
- [!] **Risco confirmado:** a extração de arquivo de dentro do zip no archive.org
      retorna **503 intermitente** (serviço pesado/rate-limited). A *listagem* do
      zip funciona. → `ArtProvider` com retry/backoff, falha graciosa e **base URL
      configurável** (mirror ou backup local descompactado servido por estático).
- [x] **`systemctl reload smbd` (não `restart`)** — apply/rollback passaram a
      recarregar o daemon (decisão 2026-06-28). **Validado em campo (2026-06-28):**
      ao ativar pelo app (só `reload`), o OPL conectou na hora e enxergou os jogos
      — o reload aplicou o share NT1 do `[global]` sem precisar de restart.
      Pendência residual: comportamento quando o `smbd` está **parado** (o app não
      liga o daemon global; o reload pressupõe Samba já habilitado) — documentar a
      mensagem ao usuário nesse cenário.
- [ ] **Status derivado da config** — `status()` agora lê `opl_share.conf` +
      `include` (sem root, arquivos world-readable). Confirmar que o
      `opl_share.conf` criado sob `pkexec` fica legível (0644) para o usuário ler o
      status sem privilégio.

## Tarefas
- [x] `core`: tipo `GameId` (normalização/validação do formato `LLLL_NNN.NN`) e
      `parse_boot2_game_id` (extrai do texto do `SYSTEM.CNF`). 6 testes.
- [x] `core`: parser ISO9660 puro (PVD → extent do diretório raiz; registros de
      diretório → `find_file`/`name_matches`). 5 testes com bytes sintéticos.
- [x] `infra`: `iso::read_game_id(path)` — traversal + leitura do `SYSTEM.CNF`,
      delegando o parse ao `core`; lê só PVD + raiz + `SYSTEM.CNF`. Testado com
      ISO mínima sintética construída no teste.
- [x] `core`/`meta`: enriquecer `GameMeta` com `game_id` (`Option<GameId>`) e
      `title` (derivado do nome via `derive_title`, convenção `<GameID>.<Título>`);
      schema v2 com `#[serde(default)]` para um cache v1 ainda carregar (§6). 5
      testes novos.
- [x] `infra`: `ArtProvider` — baixa capa por Game ID, grava em `ART/` com os
      nomes do OPL; cache (não rebaixa o que já existe); falhas sem crash. Rede
      atrás do Trait `HttpGet` (mock nos testes); `UreqClient` real com
      retry/backoff em 502/503/504; base URL configurável. 6 testes.
- [x] `ui`: listagem rica em `ListView` (título/ID/mídia/tamanho) + linha-resumo
      (contagem/total formatado) e botão "Baixar capas" na worker thread. Catálogo
      recarrega ao escolher a pasta e ao iniciar; enriquecimento lê o Game ID de
      cada ISO via `iso::read_game_id`. `scan_games_with_paths` na `infra` expõe os
      caminhos; `OplMeta::from_games` persiste o cache enriquecido.
- [x] Share: autenticação opcional usuário/senha. `core`: `ShareAuth` (`Guest`/
      `User{username}`, **sem senha** — transitória) + campo `auth` no
      `ShareConfig`. `infra/smb_script`: bloco de acesso ramificado
      (`guest ok = yes` vs `guest ok = no`+`valid users`), `smbpasswd -s -a` na
      mesma janela root (guarda de usuário existente; senha via stdin, escapada),
      `smbpasswd -x` no rollback. `infra/smb_backend`: `auth_password` transitório
      + `Debug` redigido. `ui`: toggle + campo senha + aviso do trade-off; guest
      é o padrão. 9 testes novos (smb_script 7, smb_backend 2).
- [x] Manter cobertura do `core` (parsers novos cobertos por teste).

## Critérios de aceitação
- [x] Game ID extraído corretamente de ao menos uma ISO real de PS2 (validado com
      o backup `OPL_BACKUP` do usuário em 2026-06-27).
- [x] Catálogo rico exibido na UI (título/ID/mídia/tamanho/contagem) — validado em
      execução real com `OPL_BACKUP`.
- [x] Capa baixada por Game ID e gravada em `ART/` com a nomenclatura do OPL;
      OPL reconhece. **Validado em campo (2026-06-28):** capas presentes em `ART/`
      e o OPL renderiza a arte do disco na lista. (Os slots "no image" da tela de
      *Informações* do OPL são screenshots `_SCR`, não a capa — e os campos de
      texto Gênero/Lançamento/Desenvolvedor/Descrição vêm vazios → ver Backlog.)
- [x] Autenticação opcional funciona (conexão com usuário/senha além do guest).
      Conf gerado e validado com `testparm` (Samba 4.23.x); fluxo `smbpasswd`/
      `valid users` implementado e coberto por teste. **Validado em campo
      (2026-06-28):** OPL conectou com usuário `maicom` + senha; sem senha, recusou.
- [x] Sem scraping: apenas fontes consumíveis (§7).

## Validação em campo (2026-06-28) — sessão real com PS2 + OPL
OPL **v1.2.0-beta-2012-b84c2b**, share `PS2SMB` via Ethernet ("Jogos no ETH").
- ✅ **Guest:** ativou pelo app (uma senha do Polkit), o OPL conectou na hora e
  listou os jogos.
- ✅ **Autenticado:** usuário `maicom` + senha → login OK; sem senha → recusou.
- ✅ **`reload` basta:** OPL passou a enxergar o share NT1 logo após o `reload`,
  sem restart.
- ✅ **Capas:** presentes em `ART/`, OPL renderiza a arte do disco.

### Achados a tratar (backlog pós-Fase 2)
- **Scan sem filtro de extensão** (`infra/scan.rs`): lista QUALQUER arquivo de
  `CD/`/`DVD/`, daí a entrada espúria `games — 0 MB` (3 "sem Game ID" no resumo).
  → filtrar por extensão de imagem de jogo (`.iso`, `.zso`, talvez `.cso`) e
  ignorar 0 bytes. Correção pequena e isolada.
- **"Faltando alguns" jogos:** o OPL também lista APPS/POPS e coleções de
  emulador (ex.: "230 jogos Super Nintendo", "Sonic Pack 147 ROMS") que vivem
  FORA de `CD/`/`DVD/`. Nosso scan só varre ISOs de PS2 por design — divergência
  esperada. Decidir se o catálogo deve cobrir APPS/POPS (provável Fase 3).
- **Metadados do jogo (tela "Informações" do OPL vem vazia):** Título/Gênero/
  Lançamento/Desenvolvedor/Descrição/Rating em branco. Ideia do usuário: ao
  clicar num jogo da lista, abrir um **editor in-place** (view sobreposta, sem
  mover os elementos existentes) mostrando capa, nome, nome do arquivo, Game ID
  ("slug"), hash, e campos **editáveis** do que o OPL suporta. **Imagem da ISO
  não é editável** — mas a ART (capa) é gerenciável (trocar/baixar). **A
  confirmar via PyOPLM/fonte do OPL (CLAUDE.md §7/§12):** onde o OPL persiste
  esses metadados (provável `CFG/<GameID>.cfg` e/ou base de info) e o override de
  título exibido. Não assumir verdade de fórum. → candidato a Fase 3 (ou 2.5).
- **Sem persistência de estado da UI** entre execuções: o app não salva o
  diretório-alvo nem o toggle de auth num config próprio (XDG). Só persistem o
  `opl_meta.json` (no disco-alvo) e a config/conta no nível do sistema. Avaliar um
  `~/.config/oplhost/config.json` para lembrar a última pasta.
- **Tela de Configurações com mau aproveitamento de espaço:** o campo de senha
  renderiza como caixa alta; deve ser `LineEdit` de uma linha, conteúdo
  top-alinhado, deixando espaço para os próximos itens (firewall etc.).
- **Pendência residual `reload`:** comportamento quando o `smbd` está parado (o
  app não liga o daemon) — documentar/mensagem ao usuário.
- **Persistência (resposta ao usuário):** `opl_meta.json` na raiz do disco-alvo
  (cache, apagável); share isolado em `/etc/samba/opl_share.conf` + `include` no
  `smb.conf` (removidos no "Desativar e reverter"); a senha NÃO é gravada pelo
  app — vai para o Samba do sistema via `smbpasswd` (`pdbedit -L`,
  `passdb.tdb`), e o `smbpasswd -x` a remove no rollback.

## Riscos e mitigação
- **Risco:** endpoints externos instáveis/offline. → **Mitigação:** cache local,
  falha graciosa (sem crash), pasta local de art como fallback futuro.
- **Risco:** ISOs grandes / variações de formato. → **Mitigação:** ler só os
  setores necessários (PVD + diretório raiz + `SYSTEM.CNF`).
- **Risco:** deps de rede/TLS incham o `.deb`. → **Mitigação:** crate bloqueante
  com TLS estático (`rustls`); declarar runtime-dep só se necessário.
- **Risco:** parser ISO9660 caseiro com bugs sutis. → **Mitigação:** parsers
  puros no `core` com testes de bytes sintéticos + validação com ISO real.

## Encadeamento de branches (pendente de revisão/merge)
Três branches locais, **todas partindo de `fase-2-biblioteca`** e convergindo
sobre a passada de `cargo fmt` — sem conflitos entre si. Commits **assinados
(GPG)**; nada de push/PR/merge; `main` intacta.

| Branch | Topo | Conteúdo | Base |
|--------|------|----------|------|
| `chore-cargo-fmt` | `63bbee0` | `cargo fmt --all` puro do workspace (14 arquivos, só formatação) — deixa a base fmt-clean sob style_edition 2024 | `fase-2-biblioteca` |
| `fase-2-settings-toggle-servidor` | `ef21649` | painel de Configurações + toggle único + `status` por config + `reload` + Trait sem `start`/`stop` | `chore-cargo-fmt` (rebaseada) |
| `ci-github-actions` | `9dd099f` | GitHub Actions: `fmt`/clippy/test/build **todos bloqueantes** (`continue-on-error` removido) | `chore-cargo-fmt` (rebaseada) |

- **Rebase sem dor:** `fase-2-settings-toggle-servidor` foi rebaseada com
  `-X theirs` (mantém a lógica da feature nos conflitos de formatação) e em
  seguida `cargo fmt --all` normalizou — resultado determinístico, validado por
  build/clippy/66 testes verdes. `ci-github-actions` rebaseou limpa (só toca
  `.github/`).
- **Ordem de merge sugerida:** `chore-cargo-fmt` → `fase-2-settings-toggle-servidor`
  (fast-forward, já contém o fmt) → `ci-github-actions`. Ao subir, o Actions já
  roda com o gate de `fmt` bloqueante porque a base ficou fmt-clean.
- **Pontas pré-rebase** preservadas no reflog: settings `a9c9e08`, CI `d519309`.

## Histórico
| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-27 | Plano da fase aberto; decisões de Game ID (SYSTEM.CNF) e art (download por ID) registradas | _(pendente)_ |
| 2026-06-27 | `core`: `GameId` + `parse_boot2_game_id` + parser ISO9660 puro; `infra`: `iso::read_game_id`. core 23 / infra 21 testes verdes | _(pendente)_ |
| 2026-06-27 | Pesquisa de endpoints: fonte OPLM (archive.org), estrutura `PS2/<id>/<id>_COV.jpg`, sufixos do OPL; `ureq` 3.3.0 confirmado; risco 503 registrado | _(pendente)_ |
| 2026-06-27 | `infra`: `ArtProvider` (Trait `HttpGet` + mock, `UreqClient` real com retry/backoff 502-504, base URL configurável). infra 27 testes verdes | _(pendente)_ |
| 2026-06-27 | `core`: `GameMeta` ganha `game_id`/`title`; `derive_title`; cache v2 com `serde(default)` (compat v1). core 29 testes verdes | _(pendente)_ |
| 2026-06-27 | UI: catálogo rico (`ListView` título/ID/mídia/tamanho + resumo) e botão "Baixar capas"; `scan_games_with_paths` + `OplMeta::from_games`. infra 28 testes verdes | _(pendente)_ |
| 2026-06-27 | Validação real: leitor ISO9660 + extração de Game ID + catálogo rico confirmados com o backup `OPL_BACKUP` do usuário (2 critérios de aceitação fechados) | _(pendente)_ |
| 2026-06-27 | Auth opcional usuário/senha: `core` `ShareAuth`; `infra` `valid users`/`smbpasswd` (stdin, escapado) + `smbpasswd -x` no rollback + `Debug` redigido; `ui` toggle/senha/aviso. Conf autenticado validado com `testparm`. core 29 / infra 36 testes verdes | _(pendente)_ |
| 2026-06-27 | Roteiro de teste manual do share (guest+autenticado, cliente+OPL) em `plans/roteiro-teste-manual-share.md` | _(pendente)_ |
| 2026-06-27 | Feedback de uso real → ajustes de UX: "Baixar capas" só com catálogo; janela cabe sem corte (lista absorve o espaço); dica de pasta condicional + detecção de subpasta (`is_opl_subdir_name` no core). Decisão registrada (memória): controle do servidor vai virar "aplicar/remover config + toggle" (não mexer no smbd global) — a implementar. core 30 testes verdes | _(pendente)_ |
| 2026-06-28 | Reforma Settings + toggle único: painel de Configurações em Slint (move "Acesso ao share" da tela principal); botão único Ativar/Desativar; `status` derivado de `opl_share.conf`+include; `reload` no lugar de `restart`; Trait `StorageBackend` sem `start`/`stop` (diverge de §3 do CLAUDE.md — anotado). core 30 / infra 36 testes verdes; clippy `-D warnings` limpo | _(pendente)_ |
| 2026-06-28 | CI do GitHub Actions (branch `ci-github-actions`): build/clippy/test bloqueantes, `fmt` não-bloqueante (repo ainda não fmt-clean sob style_edition 2024 do rustfmt 1.9 — passada de `cargo fmt` dedicada fica como pendência) | _(pendente)_ |
| 2026-06-28 | Passada de `cargo fmt --all` dedicada (branch `chore-cargo-fmt`, sobre `fase-2-biblioteca`): 14 arquivos reformatados, só formatação; workspace fmt-clean; 66 testes verdes | `63bbee0` |
| 2026-06-28 | Encadeamento: `fase-2-settings-toggle-servidor` e `ci-github-actions` rebaseadas sobre `chore-cargo-fmt`; gate `fmt` do CI virou bloqueante (`continue-on-error` removido). build/clippy/66 testes verdes; tree fmt-clean | settings `ef21649`, CI `9dd099f` |
| 2026-06-28 | Roteiro de teste manual alinhado à UI atual (toggle único, painel de Configurações, modelo `reload`) + 4 critérios de fechamento (capas/OPL, auth real, `reload`, 0644) | `8ddb018` |
| 2026-06-28 | **Validação em campo com PS2 + OPL v1.2.0-beta-2012-b84c2b:** guest, auth real (`maicom`/senha), `reload` e capas OK → 4 critérios fechados. Backlog pós-Fase 2 registrado (filtro de scan, metadados editáveis in-place, persistência de UI, layout de Settings) | _(este commit)_ |
