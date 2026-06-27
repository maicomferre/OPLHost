# Fase 1 — Núcleo funcional (core / infrastructure / ui)

> Primeira versão real do app. Só começa de fato após a Fase 0 ser aprovada
> (SMBv1 confirmado com PS2). O scaffold inicial (esqueleto que compila e roda)
> é montado ainda durante a Fase 0, mas os Traits derivam do que o spike provou.

- **Status:** Em andamento
- **Criado em:** 2026-06-26
- **Última atualização:** 2026-06-27 (spike portado para `SmbBackend`; adapters,
  catálogo/meta e fiação da UI feitos; chamadas ao `pkexec` movidas para worker
  thread)

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
- [x] Empacotamento `.deb`: metadata `cargo-deb`, `.desktop` e `postinst`
      validando `samba`/`polkit`. **Gerar de fato com `cargo deb` ainda pendente
      (ferramenta não instalada no ambiente).**
- [ ] Substituir o campo de texto do diretório por um seletor nativo de pasta
      (avaliar `rfd` xdg-portal vs. diálogo do Slint) — evitado agora para não
      arriscar build-deps de GTK.
- [ ] Rodar `apply`/`rollback` reais ponta a ponta com Polkit (teste manual com
      senha) e gerar/instalar o `.deb`.

## Critérios de aceitação
- [ ] App abre, inicia/para o servidor SMB e mostra IP/instruções de conexão.
- [ ] Share gerado de forma isolada e revertível; firewall e Polkit funcionando.
- [ ] Estrutura de pastas do OPL injetada no diretório-alvo.
- [ ] Cobertura do `core` ≥ 70%.

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
| 2026-06-26 | Scaffold que compila e roda: workspace 3 crates, core testado, infra com stubs, janela Slint | _(pendente)_ |
| 2026-06-27 | Spike portado para `SmbBackend` (escalador Polkit, firewall, scripts puros testados); `core` ganha `catalog`+`meta`; `JsonMetaStore`/`scan`/`net`; UI fiada ao backend; metadata `.deb`. 28 testes verdes, clippy limpo | _(pendente)_ |
| 2026-06-27 | `pkexec` (apply/rollback) movido para worker thread; UI não trava no prompt do Polkit (`upgrade_in_event_loop` + flag `busy`) | _(pendente)_ |
