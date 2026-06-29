# Fase 3 — i18n (pt-BR / en-US), abordagem híbrida

> Terceiro item da Fase 3. O `CLAUDE.md` guarda as REGRAS; este arquivo guarda o
> *porquê* e o andamento. Manter atualizado e commitar.

- **Status:** Em andamento (implementado; falta validar a troca de idioma na GUI)
- **Criado em:** 2026-06-29
- **Última atualização:** 2026-06-29

## Contexto e objetivo

Hoje toda string de UI está hardcoded em português — no `app.slint` (~50 textos
estáticos) e em `main.rs` (~45 mensagens dinâmicas: status, erros, resumo do
catálogo, avisos do editor). O objetivo é internacionalizar com **pt-BR e en-US**
embutidos no binário e, depois, traduções editáveis pela comunidade
(CLAUDE.md §2/§11).

## Decisão de arquitetura: híbrido (aprovada pelo usuário 2026-06-29)

As strings vivem em dois mundos com soluções diferentes:

- **Estáticas no `.slint`** → **Slint nativo** `@tr("…")` + traduções `.po`
  **bundladas no binário** (`with_bundled_translations` no build;
  `slint::select_bundled_translation(lang)` em runtime). Idiomático, sem virar
  ~50 properties; `.po` editável pela comunidade.
- **Dinâmicas no Rust** → **fluent** (`fluent-rs`) com `.ftl` pt-BR/en-US
  embutidos via `include_str!`. O `@tr` não cobre strings montadas em Rust, e o
  lado Rust precisa de i18n próprio de qualquer forma.

**Idioma-fonte = inglês (en-US):** os `msgid` do `@tr` e a base do fluent são em
inglês; pt-BR é tradução. Casa com a convenção gettext (`select_bundled_translation("en")`
cai no `msgid`) e evita um `.po`/`.ftl` identidade para o inglês.

## Escopo

- **Dentro:**
  - `@tr` em todas as strings do `app.slint`; `pt-BR.po` (en→pt).
  - Módulo `ui/src/i18n.rs` (fluent): carrega `en-US.ftl`+`pt-BR.ftl`, expõe
    `t(key)` e `t_args(key, args)`; detecção de locale do SO (`$LANG`/`$LC_ALL`).
  - Seleção única de idioma no start (Slint + fluent juntos), pt-BR para locales
    `pt*`, en-US caso contrário.
  - Substituir as ~45 mensagens do `main.rs` por chamadas fluent.
  - Teste de **paridade de chaves** entre pt-BR e en-US (Slint e fluent).
- **Fora (follow-up):**
  - Seletor manual de idioma nas Configurações (por ora é automático pelo SO).
  - Traduções externas em disco editáveis sem recompilar (por ora bundladas;
    a comunidade contribui via `.po`/`.ftl` no repo + rebuild).
  - Outros idiomas além de pt-BR/en-US.

## Decisões

| Data | Decisão | Justificativa | Alternativas |
|------|---------|---------------|--------------|
| 2026-06-29 | Híbrido: `@tr`/`.po` p/ Slint + fluent p/ Rust | Cada string onde é natural; `.slint` limpo; lado Rust (que precisa de i18n próprio) usa fluent honrando o CLAUDE.md | fluent em tudo (~50 properties + refatorar `.slint`, over-engineering); `@tr` em tudo (não cobre Rust, acopla ao Slint) |
| 2026-06-29 | Idioma-fonte = inglês (msgid/base em en) | Convenção gettext; "en"/vazio cai no `msgid` sem `.po` identidade | msgid em pt (exigiria `.po` en + sempre selecionar; "en" mostraria pt) |
| 2026-06-29 | Bundled translations (no binário), não `.mo` externos via gettext | Sem dependência de gettext em runtime nem arquivos soltos; "embutidos no início" (§2) | feature `gettext` (precisa libintl + `.mo` no disco); fluent também p/ o `.slint` (boilerplate) |
| 2026-06-29 | `DefaultTranslationContext::None` | `.po` sem `msgctxt` — escrevível à mão sem o `slint-tr-extractor` | ComponentName (exige contexto no `.po`, mais frágil à mão) |
| 2026-06-29 | Detecção por `$LC_ALL`/`$LANG`, sem crate extra | Linux-only (§1); basta ler env e checar prefixo `pt` | crate `sys-locale`/`locale_config` (dep a mais por pouca lógica) |

## A validar no ambiente

- [ ] `select_bundled_translation("pt-BR")` aplica o `.po` (testar trocando `$LANG`).
- [ ] Acentuação renderiza certo na fonte do Slint (já há acento na UI hoje, ok).
- [ ] O `.po` bundlado entra no binário sem o `slint-tr-extractor` instalado
      (escrito à mão, contexto None).

## Tarefas

- [x] `ui/Cargo.toml`: deps `fluent`, `unic-langid`.
- [x] `ui/build.rs`: `CompilerConfiguration::with_bundled_translations("i18n")`
      + `with_default_translation_context(None)`. (Dir consolidado em `crates/ui/i18n/`.)
- [x] `app.slint`: literais trocados por `@tr("English source")` (~34 msgids);
      defaults dinâmicos viraram "" (preenchidos via fluent).
- [x] `i18n/pt-BR/LC_MESSAGES/oplhost.po`: traduções dos `@tr`.
- [x] `ui/src/i18n.rs`: bundle fluent (en-US base + pt-BR), `t`/`t_args`,
      detecção de locale (`$LC_ALL`/`$LC_MESSAGES`/`$LANG`), seleção do idioma do
      Slint (`select_bundled_translation`), bundle por thread (sem exigir `Sync`).
- [x] `i18n/en-US.ftl` + `i18n/pt-BR.ftl`: 24 chaves das mensagens do Rust.
- [x] `main.rs`: mensagens via `t(...)`/`t_args(...)`; `i18n::init()` logo após
      `AppWindow::new()`.
- [x] Teste de paridade de chaves (pt vs en) no fluent + fallback + interpolação.

## Critérios de aceitação

- [ ] Em locale `pt_BR`, a UI inteira (estática + dinâmica) aparece em português.
      *(implementado; validar na GUI trocando `$LANG`)*
- [ ] Em locale en (ou outro), aparece em inglês. *(idem)*
- [x] Nenhuma string de UI hardcoded fora dos arquivos de tradução. *(restam só
      códigos/dados: "CD"/"DVD"/"—"/"nobody" e o log de stderr — não-UI)*
- [x] Teste garante que pt-BR e en-US têm o mesmo conjunto de chaves.
- [x] `clippy -D warnings` e `fmt` limpos. *(CI a confirmar no PR)*

## Riscos e mitigação

- **Risco:** `.po` à mão com formato inválido → string não traduz. →
  **Mitigação:** contexto None (sem `msgctxt`), teste de fumaça e revisão.
- **Risco:** chave fluent faltando num idioma → fallback/erro. →
  **Mitigação:** teste de paridade; fluent cai no `msgid`/chave se faltar.
- **Risco:** divergência entre `.po` e `.ftl` (duas fontes). →
  **Mitigação:** plano deixa claro o split; teste de paridade em cada um.

## Histórico

| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-29 | Plano criado (i18n híbrido aprovado) | `<pendente>` |
| 2026-06-29 | Implementado: `@tr`/`.po` (Slint) + fluent `.ftl` (Rust), detecção de locale, paridade testada; CLAUDE.md §2 atualizado | `<pendente>` |
