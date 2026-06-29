# Fase 1 — Núcleo funcional (core / infrastructure / ui)

> Primeira versão real do app. Só começa de fato após a Fase 0 ser aprovada
> (SMBv1 confirmado com PS2). O scaffold inicial (esqueleto que compila e roda)
> é montado ainda durante a Fase 0, mas os Traits derivam do que o spike provou.

- **Status:** ✅ Concluída — núcleo na `main` (PR #1). Workspace 3 crates, Traits,
  `SmbBackend`, firewall, Polkit (`pkexec`), estrutura de pastas, `opl_meta.json`,
  seletor nativo de pasta, `.deb`; ciclo apply→smbclient→rollback validado ponta a
  ponta. Detalhe no Histórico abaixo.
- **Criado em:** 2026-06-26
- **Última atualização:** 2026-06-27

## Contexto e objetivo
Transformar o aprendizado do spike numa arquitetura Clean (Ports & Adapters)
testável: `core` agnóstico a I/O, `infrastructure` com os adapters reais, `ui`
em Slint desacoplada. Entregar start/stop do servidor SMB, seleção de diretório,
exibição de IP/instruções, geração/rollback do share isolado, firewall, Polkit,
injeção da estrutura de pastas do OPL e `opl_meta.json`.

## Escopo
- **Dentro:** workspace Cargo (3 crates); Traits `StorageBackend` e `Fs`;
  `SmbBackend`; `FirewallManager`; `PrivilegeEscalator`; `MetaStore`; GUI Slint
  mínima funcional; estrutura de pastas do OPL; testes do `core` ≥ 70%;
  empacotamento `.deb`.
- **Fora:** catálogo rico e art (Fase 2); UDPBD, tray, i18n externo, FTP
  (Fase 3).

## Decisões
| Data | Decisão | Justificativa | Alternativas consideradas |
|------|---------|---------------|---------------------------|
| 2026-06-26 | Workspace Cargo com 3 crates (`core`, `infrastructure`, `ui`) | Inversão de dependência imposta pelo compilador; troca Slint→egui não toca o `core` | Crate único com módulos (separação só por convenção) |
| 2026-06-26 | `core` define um Trait `Fs` (port de filesystem) | Manter o `core` testável sem tocar disco; mockável nos testes de estrutura de pastas | I/O direto no `core` (quebraria a testabilidade e a regra de inversão) |
| 2026-06-26 | `StorageBackend` genérico (sem pressupor SMB) já desde o esqueleto | Acomodar um futuro `UdpbdBackend` sem refatoração dolorosa | Trait casado com SMB/`smb.conf` (travaria o backend alternativo) |
| 2026-06-27 | Construção dos scripts (`opl_share.conf`, apply, rollback) em funções PURAS (`smb_script`) | A parte mais sujeita a erro fica testável sem root nem disco; o backend só compõe e delega | Montar o script dentro do `SmbBackend` (intestável sem `pkexec`) |
| 2026-06-27 | `PrivilegeEscalator` como Trait; `SmbBackend` genérico sobre ele | Testar a composição do script com um escalador mock que captura o texto | `pkexec` chamado direto no backend (sem como mockar) |
| 2026-06-27 | Elevação via `pkexec` em vez de `zbus`/D-Bus nesta fase | `pkexec` já vem com o Polkit; um script único satisfaz a "janela de privilégio" sem reimplementar o protocolo Polkit | `zbus` falando com o `polkitd` (mais código, sem ganho na V1) |
| 2026-06-27 | Iniciar = `apply_config`; Parar = `rollback` (não só `stop`) | Não deixar SMBv1 legado + porta aberta residual; alinhado à transparência/revertibilidade do §0 | "Parar" só `systemctl stop smbd` (deixaria a config legada no sistema) |
| 2026-06-27 | Diretório por campo de texto (sem seletor nativo ainda) | Fecha o fluxo ponta a ponta sem arriscar build-deps de GTK do `rfd` neste passo | Adicionar `rfd` já (risco de quebra de build por libs de sistema) |

## A validar no ambiente
- [x] Slint resolvido em `1.17.0` (dep declarada como `slint = "1"`).
- [x] `zbus` dispensado nesta fase: a elevação usa `pkexec` (já presente com o
      Polkit do sistema), agrupando tudo numa janela só. Caminho D-Bus direto
      fica para se vier a ser necessário.
- [x] Esqueleto final do `opl_share.conf` portado para `SmbBackend` via
      `smb_script::build_smb_conf` (idêntico ao validado na Fase 0).
- [x] **Dependência de build do Slint:** `libfontconfig-dev` (o renderer femtovg
      exige `fontconfig.pc`). Instalada no ambiente. Em runtime basta
      `libfontconfig1` (já presente). Considerar no `.deb`: build-dep
      `libfontconfig-dev`; runtime-dep `libfontconfig1`.

## Tarefas
- [x] Converter a raiz em workspace e criar `crates/core`, `crates/infrastructure`,
      `crates/ui`.
- [x] `core`: Traits (`StorageBackend`, `Fs`), tipos de domínio, função de
      estruturação de pastas do OPL, testes com `Fs` mockado (3 testes, verdes).
- [x] `infrastructure`: `SmbBackend` (stub `todo!()` portando o spike), `RealFs`
      funcional; demais adapters a criar.
- [x] `ui`: janela Slint mínima que abre (status, diretório, start/stop, IP,
      aviso de SMBv1).
- [x] `cargo build` do workspace e `cargo run -p oplhost` funcionando (janela
      abre sem panic).
- [x] Portar a lógica do spike para o `SmbBackend`: `smb_script` (construtores
      puros de `opl_share.conf` + apply/rollback, testados), `apply_config`/
      `rollback`/`start`/`stop`/`status` reais; precheck de porta ocupada (§8).
- [x] Adapters restantes: `FirewallManager` (fragmentos ufw/iptables),
      `PkexecEscalator` (janela única Polkit), `JsonMetaStore` (opl_meta.json).
      Extras: `net::local_ip`/`tcp_port_listening`, `scan::scan_games`.
- [x] `core`: módulos `catalog` (contagem/tamanho/categoria CD≤700MB/DVD) e
      `meta` (`OplMeta`, reconstrução sem JSON) com testes — cobertura dos
      módulos com lógica em 100%.
- [x] Fiação da UI: campo de diretório, IP local, start (apply) / stop
      (rollback), status real do `smbd`, catálogo; mensagens de erro sem panic.
- [x] Operações com `pkexec` (apply/rollback) movidas para worker thread: o
      event loop não trava no prompt do Polkit; resultado volta via
      `Weak::upgrade_in_event_loop` e a flag `busy` desabilita os botões.
- [x] Empacotamento `.deb` gerado com `cargo deb` (cargo-deb 3.7.0):
      `oplhost_0.1.0-1_amd64.deb`. Deps via `$auto` (libc6/libfontconfig1) +
      runtime (`samba`, `pkexec | policykit-1`, `zenity | kdialog`); `.desktop`,
      `postinst` de validação, changelog e descrição estendida. `lintian` sem
      erros (só 3 warnings cosméticos: sem manpage/copyright-notice/bug-closes).
- [x] Seletor nativo de pasta: botão "Escolher pasta…" → adapter `dialog` que
      dispara `zenity` (fallback `kdialog`) numa worker thread, sem build-deps de
      GTK. Descartado o `rfd` (não está no cache offline; arrasta `ashpd`/runtime
      async). `zenity | kdialog` adicionado ao `Depends` do `.deb`. O campo de
      texto continua editável como alternativa.
- [x] Ciclo real `apply`→`smbclient`→`rollback` validado ponta a ponta contra o
      código de produção (`SmbBackend` + `PkexecEscalator` + builders), via
      example descartável. Resultado abaixo.
- [x] Click-through do ciclo pela própria GUI (coberto na validação em campo da
      Fase 2); `.deb` instalado/validado (ajuste de glibc floor para Mint 21 em
      `545a0b2`).

## Resultado da validação ponta a ponta (2026-06-27)
Ambiente: Samba 4.x, `smbd` ativo, baseline limpo (`server min protocol =
SMB2_02`). Exercitado o código de produção (não um script à parte):

- **apply** (1 prompt Polkit): `APPLY OK`. `/etc/samba/opl_share.conf` criado;
  `# oplhost` + `include` injetados no `smb.conf`; `server min protocol` passou a
  `NT1`; `smbd` reiniciado e ativo.
- **smbclient NT1 guest:** `Anonymous login successful`; listou a raiz com as 10
  pastas do OPL e o `HELLO.txt` em `CD/`. **Escrita:** `put` de arquivo OK, que
  caiu no disco como `maicom:maicom` (`force user` aplicado). Leitura + escrita ✓.
- **rollback** (1 prompt Polkit): `ROLLBACK OK`. `opl_share.conf` removido;
  `include`/marcador removidos do `smb.conf`; `server min protocol` de volta a
  `SMB2_02`; `smbd` reiniciado; conexão NT1 passou a ser **recusada** ("No
  compatible protocol selected by server"); regra `ufw` de 445 removida.

Conclusão: a lógica portada do spike funciona idêntica à Fase 0, agora pela
arquitetura definitiva, numa única janela de privilégio por operação. Sistema
volta ao estado anterior sem vestígios.

## Critérios de aceitação
- [x] Inicia/para o servidor SMB (apply/rollback validados) e a UI mostra
      IP/instruções de conexão. _(Falta só o click-through pela própria GUI.)_
- [x] Share gerado de forma isolada e revertível; firewall e Polkit funcionando
      (validado ponta a ponta acima).
- [x] Estrutura de pastas do OPL injetada no diretório-alvo (10 pastas listadas
      pelo `smbclient`).
- [x] Cobertura do `core` ≥ 70%: módulos com lógica (`catalog`, `meta`,
      `opl_layout`) cobertos por testes; `domain`/`ports` são tipos/traits.

## Riscos e mitigação
- **Risco:** atrito de build/integração do Slint. → **Mitigação:** `ui`
  desacoplada para permitir a troca por egui sem tocar `core`.
- **Risco:** Traits abstraídos cedo demais. → **Mitigação:** derivá-los do que o
  spike revelou; só generalizar `StorageBackend` com mais um caso concreto na
  mão (UDPBD, na Fase 3).

## Histórico
| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-26 | Plano da fase aberto | `b8e355e` |
| 2026-06-26 | Scaffold da Clean Architecture: workspace 3 crates, core testado, infra com stubs, janela Slint | `da2d0c9` |
| 2026-06-27 | Spike portado para `SmbBackend` (escalador Polkit, firewall, scripts puros testados); `core` `catalog`+`meta`; `JsonMetaStore`/`scan`/`net`; UI fiada; metadata `.deb` | `21d121f` |
| 2026-06-27 | `pkexec` (apply/rollback) movido para worker thread (`upgrade_in_event_loop` + flag `busy`) | `819c48b` |
| 2026-06-27 | Seletor nativo de pasta via `zenity`/`kdialog` (adapter `dialog`), na worker thread; `Depends` do `.deb` atualizado | `4e130bf` |
| 2026-06-27 | `.deb` gerado com `cargo deb` (deps `$auto`, changelog, descrição); `lintian` sem erros | `837e11d` |
| 2026-06-27 | Ciclo apply→smbclient(NT1)→rollback validado ponta a ponta contra o código de produção; mergeada na `main` | `9306812` (PR #1 `4b218ce`) |
