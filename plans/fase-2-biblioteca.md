# Fase 2 — Biblioteca e metadados

> Enriquece o produto: deixa de ser só "ligar/desligar o share" e passa a
> **entender o catálogo** (identificar jogos, nomear art, contar/medir) e a
> **consumir capas das fontes externas**. Constrói sobre o `core` da Fase 1
> (`catalog`/`meta`) sem tocar a regra de inversão de dependência.

- **Status:** Em andamento
- **Criado em:** 2026-06-27
- **Última atualização:** 2026-06-27 (Game ID + parser ISO9660 no `core`; leitor
  de ISO na `infra`)

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

## A validar no ambiente
- [ ] Crate HTTP leve e **bloqueante** (casa com a worker thread já usada): avaliar
      `ureq` (TLS via rustls, sem runtime async) vs `reqwest`. Confirmar presença
      no cache/registry e impacto no `.deb` (deps de TLS).
- [ ] Endpoints e formato de nome dos arquivos de art nas fontes (set do Kira;
      backups do OPL Manager / danielb no archive.org). Confirmar antes de fixar URLs.
- [ ] Convenção exata dos nomes de ART do OPL (sufixos: `_COV`, `_COV2L`, `_ICO`,
      `_LAB`, `_BG`, etc.) — confirmar com PyOPLM/OPL.
- [ ] Leitor ISO9660: escrever um mínimo (só PVD + diretório raiz + `SYSTEM.CNF`)
      vs usar crate. Validar a extração com pelo menos uma ISO real de PS2.
- [ ] Mecanismo de auth do Samba para o share (criar usuário via `smbpasswd -a`,
      `guest ok = no`, `valid users`) — confirmar com `testparm` no ambiente.

## Tarefas
- [x] `core`: tipo `GameId` (normalização/validação do formato `LLLL_NNN.NN`) e
      `parse_boot2_game_id` (extrai do texto do `SYSTEM.CNF`). 6 testes.
- [x] `core`: parser ISO9660 puro (PVD → extent do diretório raiz; registros de
      diretório → `find_file`/`name_matches`). 5 testes com bytes sintéticos.
- [x] `infra`: `iso::read_game_id(path)` — traversal + leitura do `SYSTEM.CNF`,
      delegando o parse ao `core`; lê só PVD + raiz + `SYSTEM.CNF`. Testado com
      ISO mínima sintética construída no teste.
- [ ] `core`/`meta`: enriquecer `GameMeta` com `game_id` e `title`; catálogo rico.
- [ ] `infra`: `ArtProvider` — baixa capa por Game ID, grava em `ART/` com os
      nomes do OPL; cache (não rebaixa o que já existe); falhas sem crash.
- [ ] `ui`: listagem rica (título/ID/mídia/tamanho + contagem/total) e ação de
      "baixar capas" (na worker thread).
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
