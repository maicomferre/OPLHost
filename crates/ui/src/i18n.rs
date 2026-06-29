//! i18n do lado Rust (strings dinâmicas) via **fluent**, parte da abordagem
//! híbrida (ver `plans/fase-3-i18n.md`): o `.slint` traduz suas estáticas com
//! `@tr`/`.po`; aqui ficam as mensagens montadas em Rust (status, erros,
//! progresso, resumo do catálogo, dicas), com `.ftl` embutidos via `include_str!`.
//!
//! O idioma é detectado do locale do SO uma vez no start ([`init`]), aplicado às
//! traduções bundladas do Slint e guardado num global. Cada thread monta seu
//! próprio `FluentBundle` sob demanda (as mensagens nascem na thread da UI e em
//! worker threads), evitando exigir `Sync` do bundle.

use std::cell::RefCell;
use std::sync::OnceLock;

use fluent::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

const EN_US_FTL: &str = include_str!("../i18n/en-US.ftl");
const PT_BR_FTL: &str = include_str!("../i18n/pt-BR.ftl");

/// Idiomas suportados embutidos (en-US é o idioma-fonte/fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    EnUs,
    PtBr,
}

impl Lang {
    /// Código BCP-47 para o `LanguageIdentifier` do fluent.
    fn bcp47(self) -> &'static str {
        match self {
            Lang::EnUs => "en-US",
            Lang::PtBr => "pt-BR",
        }
    }

    /// Código que o Slint usa para selecionar a tradução bundlada. O inglês é o
    /// `msgid` (não há `.po` en) → usa-se `"en"`, que cai no texto original.
    fn slint_code(self) -> &'static str {
        match self {
            Lang::EnUs => "en",
            Lang::PtBr => "pt-BR",
        }
    }
}

static LANG: OnceLock<Lang> = OnceLock::new();

/// Detecta o idioma do locale do SO, fixa-o no global e aplica às traduções
/// bundladas do Slint. Chamar **uma vez**, logo após `AppWindow::new()` (o Slint
/// reavalia os `@tr` reativamente quando o idioma muda).
pub fn init() {
    let lang = detect_lang();
    let _ = LANG.set(lang);
    // Erro aqui (ex.: chamado cedo demais) é inofensivo: fica no idioma-fonte.
    let _ = slint::select_bundled_translation(lang.slint_code());
}

/// Idioma efetivo (default en-US se `init` ainda não rodou).
fn current_lang() -> Lang {
    LANG.get().copied().unwrap_or(Lang::EnUs)
}

/// Lê o idioma das envs de locale do Linux (§1). Prefixo `pt` → pt-BR; senão en-US.
fn detect_lang() -> Lang {
    let raw = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if raw.starts_with("pt") {
        Lang::PtBr
    } else {
        Lang::EnUs
    }
}

thread_local! {
    /// Bundle por thread, montado sob demanda a partir do idioma global.
    static BUNDLE: RefCell<Option<FluentBundle<FluentResource>>> = const { RefCell::new(None) };
}

fn build_bundle(lang: Lang) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = lang.bcp47().parse().expect("BCP-47 válido");
    let mut bundle = FluentBundle::new(vec![langid]);
    // Sem marcas de isolamento Unicode (FSI/PDI) ao redor de args — poluem a UI.
    bundle.set_use_isolating(false);
    // en-US primeiro (fallback); o idioma alvo sobrepõe as chaves que tiver.
    add_resource(&mut bundle, EN_US_FTL);
    if lang == Lang::PtBr {
        add_resource(&mut bundle, PT_BR_FTL);
    }
    bundle
}

fn add_resource(bundle: &mut FluentBundle<FluentResource>, ftl: &str) {
    // `.ftl` embutidos são conhecidos; um erro de parser deixa o resto válido.
    let res = FluentResource::try_new(ftl.to_string()).unwrap_or_else(|(r, _)| r);
    bundle.add_resource_overriding(res);
}

fn format(key: &str, args: Option<&FluentArgs>) -> String {
    BUNDLE.with(|cell| {
        if cell.borrow().is_none() {
            *cell.borrow_mut() = Some(build_bundle(current_lang()));
        }
        let b = cell.borrow();
        let bundle = b.as_ref().unwrap();
        // Chave ausente → devolve a própria chave (sinaliza o bug sem derrubar).
        let Some(msg) = bundle.get_message(key) else {
            return key.to_string();
        };
        let Some(pattern) = msg.value() else {
            return key.to_string();
        };
        let mut errors = Vec::new();
        bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned()
    })
}

/// Traduz uma mensagem sem argumentos.
pub fn t(key: &str) -> String {
    format(key, None)
}

/// Traduz uma mensagem com argumentos (`{ $nome }` no `.ftl`). Os valores entram
/// como string para evitar formatação numérica dependente de locale.
pub fn t_args(key: &str, args: &[(&str, String)]) -> String {
    let mut fargs = FluentArgs::new();
    for (k, v) in args {
        fargs.set(*k, v.clone());
    }
    format(key, Some(&fargs))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extrai as chaves de topo de um `.ftl` (linhas `chave = …`), ignorando
    /// comentários, continuações (indentadas) e linhas em branco.
    fn keys(ftl: &str) -> std::collections::BTreeSet<String> {
        ftl.lines()
            .filter(|l| !l.starts_with([' ', '\t', '#']) && !l.trim().is_empty())
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_string()))
            .collect()
    }

    /// TRAVA de paridade: en-US e pt-BR têm exatamente o mesmo conjunto de chaves.
    /// Chave faltando num idioma viraria texto não traduzido em produção.
    #[test]
    fn ftl_en_e_pt_tem_as_mesmas_chaves() {
        let en = keys(EN_US_FTL);
        let pt = keys(PT_BR_FTL);
        let so_en: Vec<_> = en.difference(&pt).collect();
        let so_pt: Vec<_> = pt.difference(&en).collect();
        assert!(
            so_en.is_empty() && so_pt.is_empty(),
            "chaves divergentes — só em en: {so_en:?}; só em pt: {so_pt:?}"
        );
        assert!(!en.is_empty());
    }

    #[test]
    fn en_us_e_o_fallback_padrao() {
        // Sem init(), current_lang() é en-US e a tradução vem do en-US.ftl.
        assert_eq!(t("status-inactive"), "Inactive — configuration not applied");
    }

    #[test]
    fn t_args_interpola() {
        let s = t_args("msg-meta-saved", &[("id", "SLUS_200.02".to_string())]);
        assert!(s.contains("SLUS_200.02.cfg"), "veio: {s}");
    }

    #[test]
    fn chave_inexistente_devolve_a_chave() {
        assert_eq!(t("chave-que-nao-existe"), "chave-que-nao-existe");
    }
}
