# Fase 3 — Backend UDPBD (BDM) por supervisão de servidor existente

> Item da Fase 3 (CLAUDE.md §7.1 e §11). (Papel dos planos: ver `plans/README.md`.)
> Este é o ponto em que a abstração `StorageBackend` é **revisitada com os dois
> casos concretos na mão** (SMB + UDPBD), como manda o §7.1 — não antes.

- **Status:** Em andamento (Etapa 1 — primitiva de servir — feita e testada sem
  hardware; Etapa 2 — fluxo guiado montar/desmontar/formatar — definida, a fazer;
  live no PS2 pendente)
- **Criado em:** 2026-07-01
- **Última atualização:** 2026-07-02

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
  `O_RDONLY` se r/w falhar), descobre o tamanho por `ioctl` e responde a pedidos
  de **setores de 512 bytes** (`pread`/`pwrite` em `offset = setor*512`;
  `UDPBD_MAX_SECTOR_READ = 512` → 256KiB por transferência). É um **transporte de
  bloco burro**: serve uma fonte de bytes seekável (**block device `/dev/sdX` ou
  arquivo-imagem**), **NÃO uma pasta**. **Não conhece FAT32, exFAT, pastas nem
  OPL** — o código não tem nenhuma referência a filesystem. Este é o descasamento
  de modelo central desta fase (ver "Transporte de bloco vs. filesystem").
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

## Transporte de bloco vs. filesystem — quem exige o quê

**A pergunta-chave (levantada em 2026-07-02):** quem exige que o alvo seja um
device/imagem FAT32/exFAT — o `udpbd-server` ou o OPL? **São dois requisitos de
partes diferentes**, e encavalá-los foi um erro da primeira versão deste plano.

- **O `udpbd-server` exige apenas** uma fonte de bytes seekável para `open()` +
  `pread`/`pwrite` por setor. Ou seja: um **block device** ou um **arquivo
  regular** (imagem). Ele **não** exige filesystem nenhum — serviria lixo binário.
- **Quem exige FAT32/exFAT é o OPL/PS2** (lado do console), não o servidor. A
  cadeia no PS2 é:

  ```
  OPL → BDM (Block Device Manager)
          ├─ driver udpbd   → lê SETORES crus pela rede do udpbd-server
          └─ driver fatfs   → monta esses setores como FAT32/exFAT
                                └─ acha a estrutura OPL (CD/ DVD/ ART/ …) DENTRO
  ```

  O **filesystem vive no PS2**. O servidor Linux só entrega blocos.

**Contraste com o SMB:** no SMB o PS2 é cliente de **arquivos** — pede "abra o
arquivo X" e o Samba traduz do filesystem **do lado do servidor** (ext4, o que
for). No UDPBD o PS2 é cliente de **blocos** — o filesystem fica **do lado dele**.
É essa subida de nível (arquivo → bloco) que **arrasta o FAT32/exFAT para dentro
do problema** e cria as consequências abaixo.

### Consequências que a V1 do plano subestimou

1. **Estado modal com acesso exclusivo.** Para o app **arrumar** a estrutura OPL
   (criar `CD/DVD/ART`, editar metadados, escanear catálogo) ele precisa do
   filesystem **montado como pasta** no Linux. Para **servir** via `udpbd-server`
   o device precisa estar **desmontado e cru**. Montar no Linux **e** servir cru
   ao mesmo tempo = dois donos no mesmo bloco = **corrupção**. Logo `Editar`
   (montado) e `Servir` (desmontado) são **mutuamente exclusivos no tempo** — o
   app tem de orquestrar a transição, não dá para fazer as duas coisas juntas.
2. **O backend NÃO resolve isso de forma transparente.** A ideia original de "o
   app cuida disso no backend" não se sustenta: mkfs/mount/umount são operações de
   sistema com privilégio e com um estado (montado?) que a UI precisa **mostrar e
   guiar**. Vira um **fluxo assistido**, não um detalhe interno.
3. **FAT32 tem teto de 4 GiB por arquivo.** Muitas ISOs de DVD de PS2 passam de
   4 GB → **não cabem em FAT32**. Isso empurra forte para **exFAT** (sem esse
   teto) ou para split de ISO (formato antigo do OPL). Suporte a exFAT no BDM da
   versão-alvo é item de validação.
4. **Colocar ISOs na imagem = copiar arquivo grande.** O CLAUDE.md §4 diz "não
   copiar/mover arquivos grandes entre partições". Uma **imagem** gerenciada pelo
   app obriga copiar as ISOs para dentro dela (tensão direta com §4). Servir um
   **device físico** que já contém os jogos evita a cópia — por isso os dois modos
   de alvo abaixo não são equivalentes.

## UX do UDPBD (repensada em 2026-07-02)

O UDPBD deixa de ser "escolha um device e clique em ativar" e vira um **fluxo
guiado, modal**, com dois modos de alvo:

- **Modo A — device/partição físico já FAT32/exFAT (recomendado).** O usuário
  aponta uma partição (ex.: um pendrive/HD exFAT). O app a **monta** para
  organizar (mesma experiência do SMB: layout, catálogo, capas, metadados) e a
  **desmonta** para servir ao PS2. Os jogos vivem no próprio disco → **sem copiar
  para uma imagem** (respeita §4). Custo: o usuário dedica um disco FAT32/exFAT.
- **Modo B — imagem-arquivo gerenciada pelo app (conveniência).** O app **cria**
  um `.img`, **formata** (`mkfs.exfat`/`mkfs.vfat`), **loop-monta** para popular a
  estrutura, **desmonta** e serve. Custo: exige espaço para a imagem e **copiar**
  as ISOs para dentro (tensão com §4) + o teto de 4 GiB se FAT32.

**Ponte com o fluxo SMB atual (a sua ideia de "recomendar UDPBD"):** quando o
diretório-alvo do usuário **já é o ponto de montagem de uma partição FAT32/exFAT**
(`/dev/sdX1` montado), o app pode **detectar** isso e oferecer: "esta pasta é um
disco FAT32/exFAT — dá para servir também via UDPBD (mais rápido) desmontando-o
após organizar". Assim o usuário organiza a biblioteca no fluxo de sempre e o
UDPBD entra como uma opção de **servir o mesmo disco**, só que por bloco.

**Máquina de estados da UX (a implementar):**

```
        (montado)                         (desmontado)
   ┌── EDITAR ──────────────── desmontar ──────────────→ SERVIR ──┐
   │  layout/catálogo/capas/                    udpbd-server up    │
   │  metadados funcionam                       (não editável)     │
   └──────────────── montar ←─────── parar de servir ←────────────┘
```

A UI **nunca** deixa os dois estados ativos juntos; toda transição
(montar/desmontar/servir) é comunicada (§8) — inclusive o porquê ("desmontando
para o PS2 poder acessar o disco em modo bloco sem corromper").

## Escopo

Esta fase entrega a **primitiva** (supervisão do servidor) — mas, à luz da
distinção acima, a experiência **completa** do UDPBD (organizar → servir) exige
uma camada de orquestração montar/desmontar/formatar que passa a ser explícita.
Reorganizado em etapas:

- **Etapa 1 — primitiva de servir (FEITA, sem hardware):**
  - Trait verbal `apply`/`status`/`rollback` (SMB como 2º caso concreto).
  - `UdpbdBackend` supervisiona o `udpbd-server` existente (§7.1: NÃO reimplementar
    o protocolo) via unit transiente do systemd; firewall UDP 48573; escopo
    root/`--user` conforme o alvo. Testado com mock.
  - Seleção de backend na UI + status/toggle backend-aware, persistidos.
  - **Limite honesto:** serve um device/imagem **que o usuário já preparou**
    (formatado e organizado por fora). Não monta, não formata, não escaneia.

- **Etapa 2 — fluxo guiado de alvo (PRÓXIMA; onde mora a UX repensada):**
  - **Orquestração montar/desmontar** com máquina de estados `Editar`↔`Servir`,
    impedindo acesso simultâneo (anti-corrupção) e comunicando cada transição.
  - **Modo A (device físico):** detectar que o diretório-alvo é uma partição
    FAT32/exFAT montada e oferecer servir via UDPBD após desmontar; reusar o
    layout/catálogo/capas no estado montado.
  - **Modo B (fábrica de imagem):** criar `.img` + `mkfs` (**exFAT preferido** pelo
    teto de 4 GiB do FAT32) + loop-mount + popular + desmontar. **Copiar ISOs para
    a imagem** conflita com §4 → decidir política (avisar/limitar; preferir Modo A).
  - Operações de sistema (mount/umount/mkfs/losetup) agrupadas na **janela Polkit**
    (§5), como o SMB já faz.

- **Fora (follow-up):**
  - Telemetria de transferência do UDPBD (o servidor imprime KiB lidos/escritos) —
    ver `visao-produto-udp-telemetria`.
  - Auto-instalação do binário `udpbd-server` (empacotar/baixar). Hoje **detecta**
    no PATH e orienta se ausente.
  - Split de ISO >4 GiB para FAT32 (se exFAT não servir na versão-alvo do OPL).
  - `neutrino`/`udpfs` como servidores alternativos — o adapter deve permitir, mas
    o foco é o `udpbd-server`.

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
| 2026-07-02 | O `udpbd-server` é um **transporte de bloco burro**; o requisito de FAT32/exFAT é do **OPL/PS2** (driver fatfs no console), não do servidor | Verificado na fonte: o servidor só faz `open`+`pread`/`pwrite` por setor, sem nenhuma noção de filesystem. Corrige a redação anterior que dizia "serve device/imagem FAT32" como se o servidor exigisse o FS | Tratar o FS como responsabilidade do servidor (errado — levaria a desenhar o backend para "formatar antes de servir") |
| 2026-07-02 | O problema do filesystem **não** é resolvido transparente no backend; vira **fluxo guiado** com estado modal `Editar`(montado)↔`Servir`(desmontado) | Montar no Linux e servir cru ao mesmo tempo corrompe o FS (acesso exclusivo). mkfs/mount/umount são operações de sistema com estado que a UI precisa mostrar e guiar (§8) | "App cuida disso no backend" (ideia original) — inviável: esconde estado que o usuário precisa entender e arrisca corrupção silenciosa |
| 2026-07-02 | Dois modos de alvo: **A) device físico FAT32/exFAT (recomendado)** e **B) imagem-arquivo gerenciada**. Priorizar A | No Modo A os jogos vivem no próprio disco → **sem copiar** para uma imagem (respeita §4) e sem teto de tamanho. B é conveniência mas obriga cópia + espaço + (se FAT32) teto de 4 GiB | Só imagem (viola §4, exige cópia e espaço); só device (exclui quem não quer dedicar disco) |
| 2026-07-02 | **exFAT preferido** ao FAT32 para o alvo do UDPBD (validar suporte no OPL da versão-alvo) | FAT32 tem teto de **4 GiB por arquivo**; muitas ISOs de DVD passam disso e não caberiam. exFAT não tem esse teto | FAT32 + split de ISO (formato antigo; mais complexo) — fica como fallback se o OPL-alvo não ler exFAT |

## A validar no ambiente (sessão de hardware — PS2/OPL real)

> Registrados para a sessão dedicada de testes de UDPBD (o usuário roda separado).
> Nada aqui bloqueia a implementação sem-hardware (refatoração + adapter + testes).

- [ ] Qual **build do `udpbd-server`** casa com o **OPL beta-2012** do ambiente
      (risco de protocolo — ver issue de incompatibilidade de versão). Confirmar
      `udpbd-server <img>` conectando no OPL.
- [ ] Fluxo no OPL para ativar BDM/UDPBD (desabilitar SMB/ETH, IP do servidor como
      gateway, iniciar o device) — documentar o passo-a-passo real.
- [ ] Alvo real: servir uma **imagem** vs. um **/dev/sdX** — confirmar permissões
      (r/w), se precisa root, e se o OPL lê o layout.
- [ ] **exFAT vs FAT32:** o BDM do **OPL beta-2012** monta **exFAT**? (define se o
      Modo B usa `mkfs.exfat` ou cai no FAT32 + split por causa do teto de 4 GiB).
- [ ] Confirmar em campo o **teto de 4 GiB** do FAT32 com uma ISO de DVD grande
      (e se o OPL trata ISO splitada nessa versão, caso precise do fallback).
- [ ] Mecanismo de **montar/desmontar** sem fricção: precisa de root (Polkit) ou
      `udisks2`/`--user` resolve? Como detectar "montado" de forma confiável.
- [ ] Acesso exclusivo: garantir que o app **recusa servir** um alvo ainda montado
      (e vice-versa) — testar que a guarda anti-corrupção funciona.
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
- **Risco:** **corrupção** por servir um alvo ainda montado (ou montar enquanto
  serve). → **Mitigação:** máquina de estados `Editar`↔`Servir` com guarda dura —
  o app recusa a transição se o outro estado estiver ativo; comunicar o porquê.
- **Risco:** **FAT32 não comporta ISO >4 GiB** (DVDs). → **Mitigação:** preferir
  exFAT (validar no OPL-alvo); fallback split de ISO só se exFAT não servir.
- **Risco:** fábrica de imagem (Modo B) **copia ISOs grandes** para dentro,
  contra §4. → **Mitigação:** priorizar o Modo A (device físico, sem cópia);
  no Modo B, avisar do custo de espaço/tempo e nunca copiar sem confirmação.
- **Risco:** montar/formatar exige root e some com a "janela única" do §5. →
  **Mitigação:** agrupar mount/umount/mkfs no mesmo script Polkit por transição,
  como o SMB já faz; avaliar `udisks2` para o caso sem root.

## Histórico
| Data | Mudança | Commit |
|------|---------|--------|
| 2026-07-01 | Plano criado. UDPBD validado na fonte (`israpps/udpbd-server`: `udpbd-server <file>`, serve block device/imagem, UDP 48573 hardcoded, processo bloqueante). Definida a refatoração verbal do `StorageBackend` (SMB como 2º caso concreto) e a supervisão via `systemd-run` | `10b6c2a` |
| 2026-07-01 | `StorageBackend` vira contrato verbal `apply`/`status`/`rollback` (sem `ShareConfig`); SMB migrado sem mudança de comportamento | `94fde17` |
| 2026-07-01 | `BackendKind`/`UdpbdConfig` no core + `UdpbdBackend` supervisionando o `udpbd-server` (escopo condicional raw device/imagem), tudo testado com mock (sem hardware) | `73209a2` |
| 2026-07-01 | UI: seletor SMB/UDPBD nos Settings, campo de device/imagem, toggle e status backend-aware, persistência da escolha, strings pt-BR/en. Fecha a parte sem hardware (só falta o live no PS2) | `bba4dbd` |
| 2026-07-02 | Correção conceitual: `udpbd-server` é transporte de bloco; FAT32/exFAT é exigência do OPL/PS2. UX repensada — fluxo guiado modal `Editar`↔`Servir`, 2 modos de alvo (device físico preferido vs imagem), exFAT preferido pelo teto de 4 GiB, tensão de cópia com §4. Etapa 2 definida | _pendente commit_ |
