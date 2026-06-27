# Fase 1 — Núcleo funcional (core / infrastructure / ui)

> Primeira versão real do app. Só começa de fato após a Fase 0 ser aprovada
> (SMBv1 confirmado com PS2). O scaffold inicial (esqueleto que compila e roda)
> é montado ainda durante a Fase 0, mas os Traits derivam do que o spike provou.

- **Status:** Planejado
- **Criado em:** 2026-06-26
- **Última atualização:** 2026-06-26

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

## A validar no ambiente
- [ ] Versão atual do Slint (fixar `1.x`).
- [ ] Versão atual do `zbus` para Polkit/D-Bus.
- [ ] Esqueleto final do `opl_share.conf` confirmado pela Fase 0 (portar para o
      `SmbBackend`).

## Tarefas
- [ ] Converter a raiz em workspace e criar `crates/core`, `crates/infrastructure`,
      `crates/ui`.
- [ ] `core`: Traits (`StorageBackend`, `Fs`), tipos de domínio, função de
      estruturação de pastas do OPL, testes com `Fs` mockado.
- [ ] `infrastructure`: `SmbBackend` (stub portando o spike), `RealFs`, demais
      adapters como stub.
- [ ] `ui`: janela Slint mínima que abre (status, diretório, start/stop, IP).
- [ ] `cargo build` do workspace e `cargo run -p oplhost` funcionando.
- [ ] Elevar a cobertura do `core` para ≥ 70%.
- [ ] Empacotamento `.deb` com `postinst` validando `samba` e `polkit`.

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
| 2026-06-26 | Plano da fase aberto | _(pendente)_ |
