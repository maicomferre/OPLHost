# Fase 2 — Biblioteca e metadados

> Enriquece o produto: deixa de ser só "ligar/desligar o share" e passa a
> **entender o catálogo** (identificar jogos, nomear art, contar/medir) e a
> **consumir capas das fontes externas**. Constrói sobre o `core` da Fase 1
> (`catalog`/`meta`) sem tocar a regra de inversão de dependência.

- **Status:** Em andamento
- **Criado em:** 2026-06-27
- **Última atualização:** 2026-06-27 (UI: catálogo rico em `ListView` com
  título/ID/mídia/tamanho + botão "Baixar capas" na worker thread)

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
- [~] Leitor ISO9660: escrito mínimo (PVD + raiz + `SYSTEM.CNF`) e testado com ISO
      sintética. **Pendente:** validar com uma ISO real de PS2.
- [ ] Mecanismo de auth do Samba para o share (criar usuário via `smbpasswd -a`,
      `guest ok = no`, `valid users`) — confirmar com `testparm` no ambiente.
- [!] **Risco confirmado:** a extração de arquivo de dentro do zip no archive.org
      retorna **503 intermitente** (serviço pesado/rate-limited). A *listagem* do
      zip funciona. → `ArtProvider` com retry/backoff, falha graciosa e **base URL
      configurável** (mirror ou backup local descompactado servido por estático).

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
- [ ] Share: autenticação opcional usuário/senha (`ShareConfig` + `smbpasswd`),
      toggle na UI, comunicando o trade-off; mantém o guest como padrão.
- [ ] Manter cobertura do `core` (parsers novos cobertos por teste).

## Critérios de aceitação
- [ ] Game ID extraído corretamente de ao menos uma ISO real de PS2.
- [ ] Catálogo rico exibido na UI (título/ID/mídia/tamanho/contagem).
- [ ] Capa baixada por Game ID e gravada em `ART/` com a nomenclatura do OPL;
      OPL reconhece.
- [ ] Autenticação opcional funciona (conexão com usuário/senha além do guest).
- [ ] Sem scraping: apenas fontes consumíveis (§7).

## Riscos e mitigação
- **Risco:** endpoints externos instáveis/offline. → **Mitigação:** cache local,
  falha graciosa (sem crash), pasta local de art como fallback futuro.
- **Risco:** ISOs grandes / variações de formato. → **Mitigação:** ler só os
  setores necessários (PVD + diretório raiz + `SYSTEM.CNF`).
- **Risco:** deps de rede/TLS incham o `.deb`. → **Mitigação:** crate bloqueante
  com TLS estático (`rustls`); declarar runtime-dep só se necessário.
- **Risco:** parser ISO9660 caseiro com bugs sutis. → **Mitigação:** parsers
  puros no `core` com testes de bytes sintéticos + validação com ISO real.

## Histórico
| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-27 | Plano da fase aberto; decisões de Game ID (SYSTEM.CNF) e art (download por ID) registradas | _(pendente)_ |
| 2026-06-27 | `core`: `GameId` + `parse_boot2_game_id` + parser ISO9660 puro; `infra`: `iso::read_game_id`. core 23 / infra 21 testes verdes | _(pendente)_ |
| 2026-06-27 | Pesquisa de endpoints: fonte OPLM (archive.org), estrutura `PS2/<id>/<id>_COV.jpg`, sufixos do OPL; `ureq` 3.3.0 confirmado; risco 503 registrado | _(pendente)_ |
| 2026-06-27 | `infra`: `ArtProvider` (Trait `HttpGet` + mock, `UreqClient` real com retry/backoff 502-504, base URL configurável). infra 27 testes verdes | _(pendente)_ |
| 2026-06-27 | `core`: `GameMeta` ganha `game_id`/`title`; `derive_title`; cache v2 com `serde(default)` (compat v1). core 29 testes verdes | _(pendente)_ |
| 2026-06-27 | UI: catálogo rico (`ListView` título/ID/mídia/tamanho + resumo) e botão "Baixar capas"; `scan_games_with_paths` + `OplMeta::from_games`. infra 28 testes verdes | _(pendente)_ |
