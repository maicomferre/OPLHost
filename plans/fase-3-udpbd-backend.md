# Fase 3 — Backend UDPBD (BDM) por supervisão de servidor existente

> Item da Fase 3 (CLAUDE.md §7.1 e §11). (Papel dos planos: ver `plans/README.md`.)
> Este é o ponto em que a abstração `StorageBackend` é **revisitada com os dois
> casos concretos na mão** (SMB + UDPBD), como manda o §7.1 — não antes.

- **Status:** Em andamento (core + adapter + UI completos e testados sem hardware; falta só o live no PS2)
- **Criado em:** 2026-07-01
- **Última atualização:** 2026-07-01

## Contexto e objetivo

O ecossistema OPL migra para tratar rede/USB/iLink como **block devices**
intercambiáveis via BDM (Block Device Manager). O backend de rede dessa família é
o **UDPBD** (do rickgaiser): mais rápido que SMB, usa menos RAM/CPU no PS2 e sem o
buraco de segurança do SMBv1. O objetivo desta fase é o app deixar de ser só
"configurador de Samba" e virar **gerenciador de servidores OPL** — SMB e UDPBD
lado a lado, selecionáveis (visão de produto, ver `visao-produto-udp-telemetria`).

A estrutura de pastas do OPL é **idêntica** nos dois → a camada `core`
(`create_opl_layout`, catálogo, metadados, compat) é 100% reaproveitada. Muda o
**adapter** e — o ponto não-óbvio — o **modelo do alvo** (ver abaixo).

## O UDPBD — VALIDADO NA FONTE (não fórum)

Estudo de `israpps/udpbd-server` (fork mantido do original do Rick Gaiser;
CLAUDE.md §7/§12) — `main.cpp` + `udpbd.h`:

- **Invocação:** `udpbd-server <file>` — **um único argumento posicional**. Se
  `argc < 2` imprime `Usage:\n  <prog> <file>` e sai. Sem flags de porta/ACL.
- **O que serve:** abre o argumento com `open(sFileName, O_RDWR)` (cai para
  `O_RDONLY` se r/w falhar) e usa `ioctl` de block size (`DIOCGMEDIASIZE`/
  `BLKGETSIZE`) → é um **block device bruto ou arquivo-imagem** (FAT32/exFAT),
  **NÃO uma pasta**. Este é o descasamento de modelo central desta fase.
- **Porta:** `#define UDPBD_PORT 0xBDBD` = **48573**, ligada com `bind()` em
  **UDP**. Hardcoded — não há como mudar por CLI.
- **Processo:** `srv.run()` é um **loop bloqueante** que imprime estatísticas; não
  há modo daemon nem unit systemd embutido → **supervisionar é trabalho do app**.
- **Privilégio:** a porta é >1024 (sem root para o `bind`). O que exige
  privilégio é o **acesso r/w ao device**: raw `/dev/sdX` → root; imagem-arquivo
  do usuário → sem root.
- **Risco de versão de protocolo:** há relatos de builds do `udpbd-server` que só
  casam com faixas específicas de OPL/UDPBD (ex.: issue "não funciona com OPL
  2044, só 1973"). O ambiente-alvo é **OPL beta-2012** (ver
  `opl-versao-ambiente-real`) → o build do servidor que casa é item de validação.

## A grande diferença de modelo (por que a abstração é revisitada aqui)

| Aspecto | SMB (`SmbBackend`) | UDPBD (`UdpbdBackend`) |
|--------|--------------------|------------------------|
| Alvo servido | uma **pasta** Linux | um **block device / imagem** FAT32/exFAT |
| Onde vive o layout OPL | direto na pasta-alvo | **dentro** do device/imagem |
| Config | declarativa e persistente (`opl_share.conf` + include) | **processo** de longa duração a supervisar |
| Daemon | `smbd` **global** (compartilhado; nunca start/stop) | processo **dedicado** do app |
| `status` | arquivos de config presentes? | o processo está vivo? |
| Porta/proto | TCP 445 | **UDP 48573** |
| Auth | guest / usuário Samba | nenhuma |
| Privilégio | sempre (escreve em `/etc/samba`, firewall) | **condicional** (raw device sim; imagem não) |

O §7.1 já antecipava: "controle de ciclo de vida de processo só será reavaliado
com o `UdpbdBackend`". É agora — com SMB como **segundo** caso concreto validando
a abstração, não um chute a priori.

## Escopo

- **Dentro (esta fase):**
  - **Refatorar o trait `StorageBackend`** para caber nos dois modelos sem
    pressupostos de SMB (ver Decisões). Contrato verbal `apply`/`status`/
    `rollback`; cada backend carrega a **própria** config (o trait deixa de
    receber `ShareConfig`).
  - **`UdpbdBackend`** (infra) que **supervisiona o `udpbd-server` existente**
    (§7.1: NÃO reimplementar o protocolo) via **unit transiente do systemd**
    (`systemd-run`), com firewall UDP 48573. Testável com escalador mock, sem
    hardware.
  - **Seleção de backend** na UI (SMB | UDPBD) persistida em `AppSettings`;
    factory `make_backend(kind) -> Box<dyn StorageBackend>`; `status` e toggle
    passam a ser backend-aware.
  - Firewall genérico por **proto+porta** (TCP 445 / UDP 48573).
- **Fora (follow-up):**
  - **Criação/formatação de imagem FAT32/exFAT** pelo app (loop-mount, aplicar
    layout dentro, desmontar). V1 do UDPBD serve um **device/imagem já existente**
    escolhido pelo usuário; a fábrica de imagem é fase seguinte.
  - Telemetria de transferência do UDPBD (o servidor imprime KiB lidos/escritos) —
    ver `visao-produto-udp-telemetria`.
  - Auto-instalação do binário `udpbd-server` (empacotar/baixar). V1 **detecta** o
    binário no PATH e orienta se ausente.
  - `neutrino`/`udpfs` como servidores alternativos — o adapter deve permitir, mas
    a V1 mira o `udpbd-server`.

## Decisões

| Data | Decisão | Justificativa | Alternativas consideradas |
|------|---------|---------------|---------------------------|
| 2026-07-01 | Trait vira **verbal** — `apply(&self)`/`status(&self)`/`rollback(&self)` — e **deixa de receber `ShareConfig`**; cada backend guarda a própria config na construção | Remove o cheiro de "o trait conhece um tipo SMB-flavored". `SmbBackend` já guardava a `cfg` internamente (usava-a no `rollback`); o parâmetro em `apply_config` era redundante. Assim `UdpbdBackend` recebe a **sua** config sem forçar campos de SMB (`share_name`, `owner_user`, `auth`) num tipo comum | Manter `apply_config(&ShareConfig)` e o UDPBD ignorar campos (mantém o smell); OU um enum de config no parâmetro (acopla o trait a todos os backends) |
| 2026-07-01 | Manter o **contrato de 3 verbos** (não reintroduzir `start`/`stop`) — para o UDPBD `apply` = "passar a servir" (sobe o processo), `rollback` = "parar de servir" | O motivo de remover `start`/`stop` (2026-06-27) foi **não mexer no daemon global** do Samba; isso **não se aplica** a um processo dedicado. A UI já modela um **toggle único** (ativar/desativar) que mapeia 1:1 nos dois backends | `start`/`stop`/`is_alive` no trait (reintroduz API removida; duplica o toggle) |
| 2026-07-01 | Supervisionar o `udpbd-server` via **unit transiente do systemd** (`systemd-run --unit=oplhost-udpbd ...`), não como filho do processo da UI | Sobrevive ao fechar a janela (coerente com a visão de tray/background), é supervisável e o `status` sai de `systemctl is-active` — espelha o modelo "declarativo" que o SMB já tem. `run()` do servidor é bloqueante e sem daemon próprio | Filho `Command::spawn` dono da UI (morre ao fechar; contradiz tray); reimplementar UDPBD em Rust (proibido §7.1) |
| 2026-07-01 | Escopo `--user` vs system do unit **depende do alvo**: imagem/arquivo do usuário → `systemd-run --user` (sem Polkit); raw `/dev/sdX` → unit de sistema via a janela Polkit | O privilégio no UDPBD é condicional ao acesso ao device (validado na fonte: `open(O_RDWR)`), não à porta. Agrupar root numa janela só continua valendo (§5) | Sempre root (pede senha à toa no caso imagem); nunca root (falha no raw device) |
| 2026-07-01 | V1 do UDPBD serve um **device/imagem já existente** escolhido pelo usuário; **não** cria/formata imagem | Corta o maior risco/escopo (loop-mount + mkfs + aplicar layout dentro) para uma fase seguinte, entregando o caminho de supervisão/firewall/seleção antes | Fazer a fábrica de imagem já na V1 (infla a fase; adia o valor de ter UDPBD servindo) |
| 2026-07-01 | Firewall generalizado para **(proto, porta)**; UDPBD abre **UDP 48573** | Mesma rotina de `ufw`/`iptables` já existe para TCP 445; só falta o eixo de protocolo | Duplicar a rotina de firewall por backend |
| 2026-07-01 | `BackendKind { Smb, Udpbd }` em `AppSettings`; factory devolve `Box<dyn StorageBackend>`; `current_status` fica backend-aware | Um só ponto de decisão; a UI não conhece os adapters (mantém `ui` desacoplada de `infrastructure`, CLAUDE.md §3) | Hardcode do SMB (como hoje) — impede a coexistência |

## A validar no ambiente (sessão de hardware — PS2/OPL real)

> Registrados para a sessão dedicada de testes de UDPBD (o usuário roda separado).
> Nada aqui bloqueia a implementação sem-hardware (refatoração + adapter + testes).

- [ ] Qual **build do `udpbd-server`** casa com o **OPL beta-2012** do ambiente
      (risco de protocolo — ver issue de incompatibilidade de versão). Confirmar
      `udpbd-server <img>` conectando no OPL.
- [ ] Fluxo no OPL para ativar BDM/UDPBD (desabilitar SMB/ETH, IP do servidor como
      gateway, iniciar o device) — documentar o passo-a-passo real.
- [ ] Alvo real: servir uma **imagem FAT32/exFAT** vs. um **/dev/sdX** —
      confirmar permissões (r/w), se precisa root, e se o OPL lê o layout.
- [ ] O `systemd-run --user` mantém o servidor vivo após fechar o app e o
      `systemctl --user is-active oplhost-udpbd` reflete o status.
- [ ] Firewall: `ufw allow 48573/udp` de fato libera o console (e `ufw`/iptables
      persistem).
- [ ] Porta 48573 UDP: como detectar "em uso por outro serviço" de forma confiável
      (UDP não tem LISTEN como TCP) — validar a heurística escolhida.

## Tarefas

### core — refatorar a abstração (sem hardware) — ✅ `94fde17`, `73209a2`
- [x] `StorageBackend`: `apply(&self)`, `status(&self)`, `rollback(&self)` (tirar
      `ShareConfig` das assinaturas). Doc do trait explica os dois modelos e por
      que os 3 verbos servem aos dois.
- [x] `BackendKind { Smb, Udpbd }` no domínio; `AppSettings` ganha o campo
      (`#[serde(default)]` → migração sem bump de versão, default `Smb`).
- [x] Tipos de config por backend: `ShareConfig` (SMB) mantido e
      `UdpbdConfig { device }` criado (`UDPBD_PORT = 0xBDBD`).
- [x] Testes: settings com `backend_kind` (default + round-trip + config antiga).

### infra — `UdpbdBackend` (sem hardware) — ✅ `73209a2`
- [x] `udpbd_script.rs` (puro): monta `systemd-run`/rollback + firewall UDP,
      com escopo `--user` (imagem) ou sistema (raw device). Testável por string.
- [x] `UdpbdBackend` impl `StorageBackend`: `apply` sobe a unit transiente +
      firewall UDP; `rollback` para/remove a unit + fecha a porta; `status` lê
      `systemctl is-active` (tolerante). Genérico sobre `PrivilegeEscalator`
      (raw device) e `UserShell` (`--user`, imagem) — mockáveis.
- [x] `firewall.rs`/script: **já** parametrizado por **(proto, porta)** (reuso;
      teste de UDP 48573 presente).
- [x] `server_available()` (checa PATH/arquivo) → a UI avisa se ausente (§8).
- [x] Testes (escalador/shell mock): raw device → via root + `ufw 48573/udp`;
      imagem → `--user` sem Polkit nem firewall; `rollback` nos dois escopos.

### ui — seleção de backend (sem hardware) — ✅
- [x] Seletor SMB | UDPBD nos Settings (CheckBox), ligado a `backend-udpbd`
      persistido em `BackendKind`; "Share access" (auth) fica só no SMB.
- [x] `run_activate_udpbd`/`run_deactivate_udpbd` em `jobs`; `handle_toggle_server`
      ramifica por backend; `current_status` é backend-aware (lê o `config.json`)
      — fim do hardcode SMB no fluxo de status/toggle.
- [x] Campo de **device/imagem** do UDPBD (LineEdit próprio, separado do
      diretório-alvo do SMB) exibido quando esse backend está escolhido;
      persistido em `udpbd_device`.
- [x] Strings via `@tr` (idioma-fonte en) + pt-BR (`.po`) + fluent
      (`msg-choose-device`, `msg-udpbd-server-missing`): comunica que UDPBD serve
      um block device/imagem cru e abre UDP 48573 (transparência, §8/§0).
- Nota: catálogo (scan de `CD/DVD`) e "baixar capas" seguem sendo do fluxo SMB
  (pasta). No UDPBD os arquivos vivem **dentro** do device/imagem → catálogo fica
  vazio na V1 (fora de escopo; casa com a fábrica de imagem adiada).

## Critérios de aceitação

- [ ] Trait refatorado; `SmbBackend` migrado para `apply()`/config própria com
      **todos os testes atuais verdes** (não regredir SMB).
- [ ] `UdpbdBackend` compila, implementa o trait e passa nos testes de script/
      supervisão com mocks (sem hardware).
- [ ] Firewall parametrizado por proto+porta com teste cobrindo UDP 48573.
- [x] UI permite escolher o backend e o estado persiste entre sessões.
- [x] `cargo test --workspace` (122), `clippy` e `fmt` limpos.
- [ ] **Em campo (sessão de hardware):** OPL beta-2012 conecta e lê o catálogo via
      `udpbd-server` supervisionado pelo app (itens de "A validar no ambiente").

## Riscos e mitigação

- **Risco:** protocolo do `udpbd-server` não casar com o OPL beta-2012. →
  **Mitigação:** item de validação nº1; adapter agnóstico ao build (caminho do
  binário configurável); documentar a matriz de compatibilidade encontrada.
- **Risco:** o descasamento pasta↔block-device vazar para o `core`. →
  **Mitigação:** `core` só conhece o **layout** (idêntico); o "onde" (pasta vs
  device/imagem) é do adapter. V1 serve device/imagem existente; a fábrica de
  imagem fica isolada em follow-up.
- **Risco:** reintroduzir controle de daemon e quebrar o isolamento do SMB. →
  **Mitigação:** o processo do UDPBD é **dedicado** (unit própria do app), nunca
  um daemon compartilhado; SMB segue sem start/stop.
- **Risco:** detecção de porta UDP ocupada não confiável. → **Mitigação:** item de
  validação; degradar para aviso não-fatal em vez de bloquear (§8).

## Histórico
| Data | Mudança | Commit |
|------|---------|--------|
| 2026-07-01 | Plano criado. UDPBD validado na fonte (`israpps/udpbd-server`: `udpbd-server <file>`, serve block device/imagem, UDP 48573 hardcoded, processo bloqueante). Definida a refatoração verbal do `StorageBackend` (SMB como 2º caso concreto) e a supervisão via `systemd-run` | `10b6c2a` |
| 2026-07-01 | `StorageBackend` vira contrato verbal `apply`/`status`/`rollback` (sem `ShareConfig`); SMB migrado sem mudança de comportamento | `94fde17` |
| 2026-07-01 | `BackendKind`/`UdpbdConfig` no core + `UdpbdBackend` supervisionando o `udpbd-server` (escopo condicional raw device/imagem), tudo testado com mock (sem hardware) | `73209a2` |
| 2026-07-01 | UI: seletor SMB/UDPBD nos Settings, campo de device/imagem, toggle e status backend-aware, persistência da escolha, strings pt-BR/en. Fecha a parte sem hardware (só falta o live no PS2) | _pendente commit_ |
