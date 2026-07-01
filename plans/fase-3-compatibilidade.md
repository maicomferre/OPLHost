# Fase 3 — Listas de compatibilidade por jogo (`$Compatibility` no `CFG/<GameID>.cfg`)

> Item da Fase 3 (CLAUDE.md §11). (Papel dos planos: ver `plans/README.md`.)

- **Status:** Em andamento (core+infra+UI implementados; falta validar no PS2/OPL real)
- **Criado em:** 2026-06-30
- **Última atualização:** 2026-06-30

## Contexto e objetivo

O OPL tem, por jogo, **modos de compatibilidade** (a tela "Game Settings"). São
flags que ajustam como o jogo é carregado (leitura de disco, vídeos, IGR…).
Vários jogos só rodam bem com um modo específico marcado. Hoje o usuário precisa
mexer nisso no próprio PS2; o objetivo é **deixar marcar/desmarcar esses modos
pelo oplhost**, gravando no mesmo lugar que o OPL lê.

Reaproveita ~tudo da feature do editor de metadados (`plans/fase-3-editor-
metadados.md`): o `$Compatibility` mora **no mesmo `CFG/<GameID>.cfg`**, que o
`GameCfg` (core) já lê/grava por **read-modify-write** preservando chaves `$`.

UX: a mesma overlay in-place do editor de metadados ganha uma seção
"Compatibilidade" com checkboxes (um por modo). Carrega ao abrir o jogo, grava no
Salvar junto com os campos de info.

## O formato do OPL — VALIDADO NA FONTE (não fórum)

Estudo de `ps2homebrew/Open-PS2-Loader@master` (CLAUDE.md §7/§12):

- **Chave:** `CONFIG_ITEM_COMPAT = "$Compatibility"` (`include/config.h`). Mesmo
  `.cfg` por jogo, com prefixo `$` (config do OPL, não rótulo de info).
- **Valor:** **bitmask decimal cru**, sem bit de "configurado"/offset. Lido com
  `configGetInt(..., &compatmask)` e testado direto: `if (compatmask &
  COMPAT_MODE_1) …` (`src/supportbase.c`).
- **Bits** (`include/iosupport.h`) e **rótulos** (`lng_tmpl/_base.yml`):

  | Bit | Macro | Mode # | Rótulo nativo (en) |
  |-----|-------|--------|--------------------|
  | `0x01` | `COMPAT_MODE_1` | Mode 1 | Accurate Reads |
  | `0x02` | `COMPAT_MODE_2` | Mode 2 | Synchronous Mode |
  | `0x04` | `COMPAT_MODE_3` | Mode 3 | Unhook Syscalls |
  | `0x08` | `COMPAT_MODE_4` | Mode 4 | Skip Videos |
  | `0x10` | `COMPAT_MODE_5` | Mode 5 | Emulate DVD-DL |
  | `0x20` | `COMPAT_MODE_6` | Mode 6 | Disable IGR |
  | `0x40` | `COMPAT_MODE_7` | Mode 7 | **Unused** |
  | `0x80` | `COMPAT_MODE_8` | Mode 8 | **Unused** |

  `COMPAT_MODE_COUNT = 6` → a UI base do OPL só expõe os 6 primeiros. Modes 7/8
  existem como bits mas não têm uso/rótulo.

## Escopo

- **Dentro (V1):**
  - `core` puro `compat.rs`: tipo `CompatFlags` (envelopa o `u8` do bitmask;
    preserva bits desconhecidos), enum `CompatMode` (os 6 modos em uso, com
    `bit()`, número 1–6 e rótulo en de referência), `parse`/`to_config_value`,
    `is_set`/`set`. `GameCfg::compat()` e `GameCfg::apply_compat(&CompatFlags)`.
  - `infra`: estender `GameInfoStore` com `load_compat`/`save_compat` (read-
    modify-write, igual ao info).
  - UI: seção "Compatibilidade" com 6 checkboxes na overlay do editor existente;
    carrega ao abrir, grava no Salvar. Strings via Slint `@tr` (idioma-fonte en).
- **Fora (follow-up):**
  - Modes 7/8 (sem uso no OPL base) — só **preservados** se já vierem setados,
    nunca expostos para marcar.
  - Outras configs `$` por jogo (`$VMC`, `$DNAS`, `$ForceGSM`…) — preservadas,
    não editadas nesta V1.
  - Importar listas de compatibilidade da comunidade (DB externo) — fase tardia.

## Decisões

| Data | Decisão | Justificativa | Alternativas |
|------|---------|---------------|--------------|
| 2026-06-30 | Persistir em `$Compatibility` no `CFG/<GameID>.cfg` como bitmask decimal cru | Validado na fonte (`config.h`/`iosupport.h`/`supportbase.c`): é exatamente o que o OPL lê com `configGetInt` e `& COMPAT_MODE_n` | Arquivo/seção separada (o OPL não leria) |
| 2026-06-30 | `CompatFlags` envelopa o `u8` inteiro e **preserva bits desconhecidos** (7/8) | RMW também no nível de bit: não estragar um valor que já traga bits que a UI não expõe | Mascarar só os 6 bits (apagaria 7/8 alheios) |
| 2026-06-30 | Bitmask 0 → **remove** a chave `$Compatibility` | Mesma regra de "campo vazio remove a chave" do editor de info; mantém o `.cfg` limpo e o OPL cai no padrão | Gravar `$Compatibility=0` (polui; ambíguo) |
| 2026-06-30 | Estender `GameInfoStore` com `load_compat`/`save_compat` (aditivo) em vez de bundlar info+compat numa struct | Mudança aditiva: não mexe nas assinaturas/testes do info já existentes; cada método tem responsabilidade única | Aggregate `GameConfig{info,compat}` (1 RMW, mas quebra assinaturas atuais) |
| 2026-06-30 | Salvar info e compat em **dois** read-modify-write no Salvar | Arquivo local pequeno; o próprio plano do editor já roda o save na thread da UI por ser barato. Duas passadas continuam triviais e mantêm os métodos simples | 1 passada combinada (acopla os dois concerns) |
| 2026-06-30 | Rótulos dos modos via Slint `@tr` na UI; core guarda só o rótulo en de referência | i18n híbrida (CLAUDE.md §2): strings estáticas da UI são `@tr`/.po, idioma-fonte en | Hardcode pt-BR no core (fura a i18n) |

## A validar no ambiente

- [ ] Marcar um modo, salvar, e o OPL refletir o modo na tela "Game Settings"
      (PS2/OPL real — ver `opl-versao-ambiente-real`).
- [ ] Confirmar que marcar compat **não apaga** os 5 campos de info nem outras
      chaves `$` (read-modify-write nos dois sentidos).
- [ ] Conferir o significado prático de cada modo no OPL desta versão (beta-2012)
      — os rótulos vêm do `_base.yml` do master; reconfirmar se divergir.

## Tarefas

### core (`compat.rs`) — testes junto
- [x] `CompatMode` (6 variantes) com `bit() -> u8`, `number() -> u8` (1–6),
      `label() -> &'static str` (en), `ALL: [CompatMode; 6]`.
- [x] `CompatFlags(u8)`: `parse(&str)` (decimal; vazio/inválido → 0),
      `to_config_value() -> Option<String>` (None se 0), `is_set`/`set`,
      `is_empty`. Preserva bits desconhecidos no `u8`.
- [x] `GameCfg::compat() -> CompatFlags` e `apply_compat(&CompatFlags)`
      (set decimal ou remove a chave; preserva o resto).
- [x] `CONFIG_ITEM_COMPAT = "$Compatibility"`.
- [x] Testes: parse decimal; preserva bits 7/8; round-trip; `apply_compat`
      preserva info + outras `$`; 0 remove a chave; trava cruzada (apply_compat
      não toca info e vice-versa).

### infra (`fs_game_info_store.rs`)
- [x] `load_compat`/`save_compat` no trait + impl (RMW via `GameCfg`).
- [x] Testes (tempdir): save_compat preserva info/`$VMC` no disco; 0 limpa a
      chave; load de `.cfg` ausente → vazio.

### ui (seção na overlay existente)
- [x] `app.slint`: seção "Compatibilidade" com 6 `CheckBox` na overlay do editor;
      propriedades in/out por modo (ou um array); strings via `@tr`.
- [x] `main.rs`/`handlers.rs`: ao abrir o jogo, `load_compat` e popular os checks;
      no Salvar, ler os checks → `CompatFlags` → `save_compat` junto do info.
- [x] `.po`/.ftl: traduções pt-BR dos 6 rótulos + título da seção.

## Critérios de aceitação

- [x] Abrir um jogo mostra os 6 modos com o estado atual lido do `.cfg`.
- [x] Marcar/desmarcar e salvar grava `$Compatibility=<bitmask>` correto.
- [x] Salvar compat **preserva** info e demais chaves `$` (teste-trava).
- [x] Desmarcar tudo remove a chave `$Compatibility`.
- [x] Bits 7/8 pré-existentes sobrevivem a um save (teste-trava).
- [x] Testes do `core` cobrindo parse/apply/preservação.
- [ ] **Em campo:** o OPL reflete os modos salvos (PS2 real).

## Riscos e mitigação

- **Risco:** apagar info/`$VMC`/bits 7/8 ao gravar compat. → **Mitigação:** RMW +
  `CompatFlags` sobre o `u8` inteiro + testes-trava nos dois sentidos.
- **Risco:** rótulos divergirem nesta versão do OPL. → **Mitigação:** rótulos
  marcados como "validar no ambiente"; vêm do `_base.yml` do master.

## Histórico

| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-30 | Plano criado (formato `$Compatibility` validado na fonte: bitmask decimal cru, 6 modos em uso) + `compat.rs` (core, 9 testes), `load_compat`/`save_compat` (infra, 4 testes) e seção de 6 checkboxes na overlay do editor + traduções pt-BR | _pendente commit_ |
