# Fase 3 — Persistência de estado da UI (XDG) + polish de empacotamento (ícone/.desktop)

> Primeiro item da Fase 3. O `CLAUDE.md` guarda as REGRAS; este arquivo guarda o
> *porquê* e o andamento. Manter atualizado e commitar.

- **Status:** Planejado
- **Criado em:** 2026-06-29
- **Última atualização:** 2026-06-29

## Contexto e objetivo

Hoje o app **esquece tudo a cada abertura**: o usuário reescolhe o diretório-alvo
e reconfigura a autenticação toda vez. Foi uma dor levantada em campo
("esses dados persistem aonde?"). O objetivo é lembrar as últimas escolhas
**não-sensíveis** da UI entre execuções, gravando num arquivo de config do
usuário no padrão XDG.

Em paralelo (item curto e relacionado a empacotamento), fechar duas lacunas do
`.deb` descobertas agora: o app aparece no menu, mas **sem ícone** e com `Name`
minúsculo. Já existem PNGs em `images/` (na verdade 248×257, não 32×32 como o
nome sugere).

## Escopo

- **Dentro:**
  - Persistir estado de UI **não-sensível**: último diretório-alvo, flag
    "exigir autenticação" e o **nome de usuário** do share.
  - Local: `$XDG_CONFIG_HOME/oplhost/config.json` (fallback `~/.config/oplhost/`).
  - Novo **port** no `core` (`SettingsStore`) + adapter no `infrastructure`,
    mantendo a UI desacoplada (regra Slint→egui).
  - Carregar no start (pré-preencher campos) e salvar quando o estado muda.
  - Empacotamento: instalar ícone em `hicolor`, referenciar `Icon=` no
    `.desktop`, corrigir `Name=OPLHost`.
- **Fora:**
  - **Senha nunca é gravada** pelo app. Ela vive no Samba do sistema
    (`passdb.tdb`, via `smbpasswd`), não em config nossa — manter assim.
  - Não persistir catálogo/capas (isso é o `opl_meta.json`, §6, no diretório-alvo).
  - Sem migração de formato/versionamento de schema complexo nesta fase (campo
    `version` simples basta).

## Decisões

| Data | Decisão | Justificativa | Alternativas consideradas |
|------|---------|---------------|---------------------------|
| 2026-06-29 | Persistir só estado não-sensível (dir, toggle auth, usuário) | Senha pertence ao Samba do sistema; gravá-la em JSON plano seria um vazamento. Espelha a separação já existente | Guardar tudo (rejeitado: senha em claro); usar keyring/libsecret (escopo maior, fica p/ depois se necessário) |
| 2026-06-29 | Novo port `SettingsStore` no core + adapter no infra | Mantém UI desacoplada e o core testável (mesmo padrão do `MetaStore`/`opl_meta.json`) | UI ler/gravar arquivo direto (quebra a regra de isolamento de camadas) |
| 2026-06-29 | Local = `$XDG_CONFIG_HOME/oplhost/config.json`, fallback `~/.config` | Padrão XDG, separado do `opl_meta.json` (que é portátil, no disco do jogo). Config da app pertence ao usuário, não ao disco-alvo | Gravar junto do binário (não-portátil/sem permissão); junto do `opl_meta.json` (mistura conceitos) |
| 2026-06-29 | Resolver XDG via env vars (sem novo crate) — reavaliar `directories` se ficar feio | CLAUDE.md pede evitar deps especulativas; `$XDG_CONFIG_HOME`/`$HOME` resolvem | crate `directories`/`dirs` (conveniente, mas dep a mais por pouca lógica) |
| 2026-06-29 | App robusto a config ausente/corrompida → defaults, sem crash | Mesma regra do `opl_meta.json` (§6): config é conveniência, nunca fonte de verdade | Tratar como obrigatória (rejeitado) |
| 2026-06-29 | Ícone instalado em `hicolor/256x256/apps/oplhost.png` + `Icon=oplhost` | Padrão de icon theme do freedesktop; resolve por nome, sem caminho absoluto | `Icon=/caminho/absoluto` (funciona, menos idiomático); scalable/SVG (não temos SVG) |

## A validar no ambiente

- [ ] `$XDG_CONFIG_HOME` respeitado e fallback `~/.config` em GNOME/Cinnamon/MATE/XFCE.
- [ ] Ícone aparece no menu após `update-desktop-database`/refresh do cache
      (verificar se precisa de `gtk-update-icon-cache` no `postinst` ou se o DE
      pega sozinho ao instalar em `hicolor`).
- [ ] Recorte do PNG 248×257 → 256×256 quadrado sem distorcer o logo
      (confirmar com o usuário qual recorte/zoom).

## Tarefas

### Persistência (núcleo do item #1)
- [ ] `core`: struct `AppSettings` (serde) com `last_target_dir: Option<PathBuf>`,
      `auth_required: bool`, `auth_username: Option<String>`, `version: u32`.
- [ ] `core/ports.rs`: trait `SettingsStore { fn load() -> AppSettings; fn save(&AppSettings) -> Result<...> }`
      (ou métodos no padrão dos ports existentes).
- [ ] `core`: testes de (de)serialize, default quando ausente, e ignorar JSON
      corrompido retornando default.
- [ ] `infrastructure`: `FsSettingsStore` resolvendo o dir XDG, criando-o se
      faltar, lendo/gravando `config.json` via `serde_json` (já é dep).
- [ ] `ui/main.rs`: carregar no start e pré-preencher diretório + bloco de auth;
      salvar ao aplicar config / trocar diretório / mudar auth.
- [ ] Definir gatilho de save (decisão de implementação: ao "Ativar servidor" e
      ao escolher diretório; evitar gravar a cada tecla).

### Polish de empacotamento (ícone + .desktop)
- [ ] Recortar `images/large-icon.png` (248×257) para `oplhost.png` 256×256
      quadrado (confirmar recorte com o usuário).
- [ ] Adicionar asset em `crates/ui/Cargo.toml`:
      `["packaging/oplhost.png", "usr/share/icons/hicolor/256x256/apps/oplhost.png", "644"]`.
- [ ] `.desktop`: adicionar `Icon=oplhost`; corrigir `Name=OPLHost`.
- [ ] Avaliar `gtk-update-icon-cache`/`update-desktop-database` no `postinst`.
- [ ] Rebuild `cargo deb -p oplhost` e validar instalação local (ícone + nome no menu).

## Critérios de aceitação

- [ ] Reabrir o app pré-preenche o último diretório-alvo e o estado de auth
      (toggle + usuário). Senha continua sendo digitada (não é guardada).
- [ ] `config.json` **nunca** contém senha.
- [ ] App funciona normalmente se `config.json` estiver ausente ou corrompido
      (defaults, sem crash).
- [ ] Testes do `core` cobrindo (de)serialize e o caminho de default.
- [ ] Após instalar o `.deb`, o app aparece no menu **com o ícone** e o nome
      **OPLHost**.

## Riscos e mitigação

- **Risco:** gravar senha por engano no JSON. → **Mitigação:** `AppSettings` não
  tem campo de senha; teste garante ausência.
- **Risco:** cache de ícone do DE não atualizar na instalação. → **Mitigação:**
  validar e, se preciso, chamar `gtk-update-icon-cache` no `postinst`.
- **Risco:** distorção ao forçar 256×256 num PNG 248×257. → **Mitigação:**
  recorte (não esticar) e revisão visual com o usuário.

## Histórico

| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-29 | Plano criado (persistência XDG + polish ícone/.desktop) | `<pendente>` |
