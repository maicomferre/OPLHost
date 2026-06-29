use slint_build::{CompilerConfiguration, DefaultTranslationContext};

fn main() {
    // i18n híbrido (ver plans/fase-3-i18n.md): as strings estáticas do .slint usam
    // `@tr("English source")` e as traduções `.po` ficam BUNDLADAS no binário
    // (sem gettext em runtime nem .mo soltos). Em runtime, o idioma é escolhido
    // com `slint::select_bundled_translation(lang)`. Os `.po` ficam em
    // `i18n/<lang>/LC_MESSAGES/oplhost.po` (junto dos `.ftl` do fluent). Contexto
    // padrão = None para que os `.po` possam ser escritos à mão (sem `msgctxt`,
    // sem o slint-tr-extractor).
    let config = CompilerConfiguration::new()
        .with_bundled_translations("i18n")
        .with_default_translation_context(DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("falha ao compilar app.slint");
}
