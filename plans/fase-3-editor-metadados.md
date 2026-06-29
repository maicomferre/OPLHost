# Fase 3 — Editor de metadados do jogo (info do OPL: `CFG/<GameID>.cfg`)

> Segundo item da Fase 3. O `CLAUDE.md` guarda as REGRAS; este arquivo guarda o
> *porquê* e o andamento. Manter atualizado e commitar.

- **Status:** Em andamento (core+infra+UI implementados; falta validar a gravação no PS2/OPL real)
- **Criado em:** 2026-06-29
- **Última atualização:** 2026-06-29

## Contexto e objetivo

Na tela "Informações" do OPL, vários jogos vêm com **Título/Gênero/Lançamento/
Desenvolvedor/Descrição vazios** (levantado em campo, 2026-06-28). O objetivo é
deixar o usuário **preencher esses campos pelo oplhost**, gravando-os onde o OPL
de fato lê — sem mexer no conteúdo da ISO (que é imutável).

UX desejada (memória `feature-editor-metadados-jogo`): clicar num jogo da lista
abre um **editor in-place sobreposto** (mesmo padrão do painel de Configurações
com o gear — não reflui a tela). Mostra capa, nome, arquivo, Game ID e os campos
editáveis. Salvar grava no `.cfg` do jogo.

## O formato do OPL — VALIDADO NA FONTE (não fórum)

Estudo de `ps2homebrew/Open-PS2-Loader@master` (CLAUDE.md §7/§12):

- **Arquivo:** `CFG/<GameID>.cfg` na raiz do dispositivo-alvo (`src/opl.c:632`,
  `:1939`, `:2019`). `GameID` = "startup" do disco, ex.: `SLUS_200.02` →
  `CFG/SLUS_200.02.cfg`. (Casa com o nosso `GameId`.)
- **Formato:** INI `chave=valor`, uma por linha, separador `=`, whitespace à
  esquerda ignorado (`src/config.c` `splitAssignment`/`configReadFileBuffer`).
- **Chaves de info (puras, sem prefixo):** o theme engine lê via `configGetStr`
  usando o nome do atributo do tema (`src/themes.c:146-157`); os 5 campos que o
  OPL **rotula nativamente** (`lng_tmpl/_base.yml`: `INFO_TITLE/GENRE/RELEASE/
  DEVELOPER/DESCRIPTION`) são **`Title`, `Genre`, `Release`, `Developer`,
  `Description`**. `Title` sobrescreve o nome exibido.
- **Compatibilidade no MESMO arquivo:** as configs de jogo do OPL (`$Compatibility`,
  `$VMC_0`, `$DNAS`, etc.) também moram nesse `.cfg`, com **prefixo `$`**. →
  **Gravar é read-modify-write:** preservar todas as outras chaves; mexer só nos
  nossos 5 campos. Nunca reescrever o arquivo inteiro.
- **Limite:** valor ≤ **255** chars (`CONFIG_KEY_VALUE_LEN 256`), chave ≤ 31. Vale
  p/ a Descrição (truncar/avisar). Valor é 1 linha (sem `\n`).

## Escopo

- **Dentro (V1):**
  - Novo módulo `core` puro `game_info.rs`: tipo `GameInfo` (os 5 campos
    opcionais), parser/serializador `GameCfg` que **preserva** chaves
    desconhecidas/compat e ordem, `apply_info` (set/remove só dos 5 campos),
    validação (≤255, sem newline). Port `GameInfoStore`.
  - Adapter `infra` `FsGameInfoStore`: lê/grava `CFG/<id>.cfg` por
    read-modify-write; ausente → info vazia; cria `CFG/` no save.
  - UI: clicar numa linha do catálogo abre o editor sobreposto (capa read-only,
    nome/arquivo/Game ID, 5 campos editáveis). Salvar → grava o `.cfg`.
- **Fora (follow-up):**
  - Editar/baixar capa (ART) **dentro** do editor — já há `ArtProvider`; fica
    para depois. V1 mostra a capa read-only (se existir em `ART/`).
  - Campos não-rotulados nativamente (Players/Rating/Aspect/Scan): existem em
    temas da comunidade, mas o OPL base não os exibe → fora da V1.
  - Editar conteúdo da ISO (imutável). Editar jogos **sem Game ID** (sem como
    nomear o `.cfg`): o editor abre read-only e explica.

## Decisões

| Data | Decisão | Justificativa | Alternativas consideradas |
|------|---------|---------------|---------------------------|
| 2026-06-29 | Persistir em `CFG/<GameID>.cfg`, chaves `Title/Genre/Release/Developer/Description` | Validado na fonte do OPL (não fórum): é onde o theme engine lê o info | `conf_game.cfg` consolidado com blocos por prefixo (existe, mas o per-game `CFG/` é o caminho do OPL Manager/comunidade e o que o OPL carrega por jogo) |
| 2026-06-29 | Gravar por **read-modify-write** preservando chaves desconhecidas | O mesmo `.cfg` guarda `$Compatibility`/`$VMC`; reescrever apagaria ajustes do usuário | Reescrever só com nossos campos (rejeitado: destrói config de compat) |
| 2026-06-29 | `GameCfg` (parse/serialize/apply) puro no `core`; I/O no adapter | Mesma divisão do `MetaStore`/`JsonMetaStore`: lógica testável sem disco | Parsear no adapter (perde testabilidade no core) |
| 2026-06-29 | V1 só os 5 campos rotulados nativamente | É o que o OPL base mostra na tela "Informações"; Players/Rating dependem de tema | Incluir todos os campos de temas da comunidade (escopo inflado, sem efeito no OPL padrão) |
| 2026-06-29 | Campo vazio na UI = chave **removida** do `.cfg` (não `Chave=`) | Mantém o arquivo limpo e o OPL cai no comportamento padrão (nome derivado) | Gravar `Chave=` vazio (polui o arquivo; ambíguo) |
| 2026-06-29 | Editor é overlay in-place (como o painel de Settings), não nova janela | Pedido explícito do usuário; consistente com o padrão atual | Janela separada (quebra o padrão; mais código de janela) |
| 2026-06-29 | Jogo sem Game ID → editor read-only explicando | Sem `GameID` não há nome de `.cfg` p/ o OPL casar | Derivar ID na hora (fora de escopo; já há "Baixar capas" lendo ISO) |
| 2026-06-29 | Sanitizar o campo Lançamento: canonizar data p/ AAAA-MM-DD, barrar data-like inválida, deixar texto livre passar | O OPL exibe `Release` **verbatim** (não parseia — `src/themes.c` só mapeia o rótulo), então não quebra; mas o usuário pediu evitar gravar "data" sem sentido (ex.: `2007-03-132`). Heurística: só-dígitos+separadores = tentativa de data (valida/canoniza); com letras = texto livre (ano por extenso etc.) | Validação estrita rejeitando tudo que não seja AAAA-MM-DD (hostil: muitos só sabem o ano); nenhuma sanitização (deixaria gravar lixo) |

## A validar no ambiente

- [ ] Editar um jogo, salvar, e o OPL exibir os campos na tela "Informações"
      (validação em campo no PS2/OPL — ver `opl-versao-ambiente-real`).
- [ ] Confirmar que gravar info **não apaga** `$Compatibility`/`$VMC` já
      existentes no `.cfg` (read-modify-write).
- [ ] Acentuação/encoding: o OPL exibe UTF-8 nesses campos? (testar "Coração",
      "Descrição" com acento). Se não, avaliar fallback.

## Tarefas

### core (`game_info.rs`) — testes junto
- [x] `GameInfo { title, genre, release, developer, description: Option<String> }`
      + constantes das 5 chaves + `OPL_VALUE_MAX_LEN = 255`.
- [x] `GameCfg`: `parse(&str)` preservando ordem e linhas desconhecidas;
      `get/set/remove`; `info()`; `apply_info(&GameInfo)`; `Display`/`to_string`.
- [x] `GameInfo::validate()` (≤255, sem `\n`/`\r`) → erros por campo p/ a UI.
- [x] Port `GameInfoStore { load(&GameId)->Result<GameInfo,_>; save(&GameId,&GameInfo)->Result<(),_> }`
      + `GameInfoError`.
- [x] Testes (12): parse round-trip; **apply_info preserva `$Compatibility`** (trava);
      remover campo vazio; validação de 256 chars e newline; arquivo ausente → vazio.

### infra (`fs_game_info_store.rs`)
- [x] `FsGameInfoStore::new(target_dir)`; `load` lê `CFG/<id>.cfg` (ausente→vazio);
      `save` read-modify-write criando `CFG/`. Testes (4) com tempdir, incl. trava
      de preservação no disco.

### ui (overlay in-place)
- [x] `app.slint`: linhas do catálogo clicáveis (`TouchArea` → `game-clicked(int)`,
      com hover); overlay `show-game-editor` com capa (read-only), nome/arquivo/
      Game ID e os 5 campos (`LineEdit` + `TextEdit` p/ descrição); botões
      Salvar/Cancelar; aviso de limite/sem-ID; dica de descoberta no catálogo.
- [x] `main.rs`: mapeia índice→linha pelo model do Slint; ao clicar,
      `FsGameInfoStore.load` e preenche; ao salvar, valida + `save` (I/O local
      rápido, na thread da UI), atualiza o título da linha e fecha o editor.
- [x] Capa: `ART/<id>_COV.{png,jpg}` carregada via `slint::Image::load_from_path`.

> Decisão de refino: o save roda na thread da UI (gravação de 1 arquivo local
> pequeno, sem Polkit/rede), diferente de apply/rollback (que bloqueiam no
> Polkit e por isso vão p/ worker thread). Se algum alvo lento (rede/USB) tornar
> isso perceptível, mover para `spawn_job`.

## Critérios de aceitação

- [x] Clicar num jogo abre o editor in-place sem refluir a tela; Cancelar volta.
      *(implementado; validar visualmente na GUI)*
- [x] Preencher os 5 campos e salvar grava `CFG/<id>.cfg` com as chaves corretas.
      *(teste `save_cria_cfg_dir_e_grava_os_campos`)*
- [x] Gravar **preserva** chaves de compatibilidade pré-existentes (teste-trava
      em core e no disco).
- [x] Campo > 255 chars é barrado com aviso; campo vazio remove a chave.
      *(validação no save; `apply_info` remove `None`)*
- [x] Jogo sem Game ID abre read-only com explicação (sem crash).
- [x] Testes do `core` cobrindo parse/apply/validação/preservação.
- [ ] **Em campo:** o OPL exibe os campos salvos na tela "Informações" (PS2 real).

## Riscos e mitigação

- **Risco:** sobrescrever e apagar `$Compatibility`/`$VMC` do usuário. →
  **Mitigação:** read-modify-write + teste-trava que injeta `$Compatibility` e
  confere que sobrevive ao `apply_info`.
- **Risco:** OPL não exibir acento (encoding). → **Mitigação:** validar em campo;
  gravar UTF-8 e, se falhar, avaliar transliteração opcional.
- **Risco:** valor > 255 quebrar o parse do OPL. → **Mitigação:** validar/truncar
  no core antes de gravar.

## Histórico

| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-29 | Plano criado; formato do OPL (`CFG/<id>.cfg`, 5 chaves) validado na fonte | `<pendente>` |
| 2026-06-29 | `game_info.rs` (core) + `FsGameInfoStore` (infra) + editor in-place na UI; 16 testes novos, clippy/fmt limpos | `85d60a2` |
| 2026-06-29 | Sanitização do Lançamento (`normalize_release`): canoniza/valida data, +7 testes | `<pendente>` |
