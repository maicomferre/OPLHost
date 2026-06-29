//! Dica contextual sobre o diretório-alvo escolhido. Leituras de `stat` são
//! instantâneas, então roda na thread da UI.

use std::path::{Path, PathBuf};

use oplhost_core::is_opl_subdir_name;

use crate::AppWindow;
use crate::i18n::{t, t_args};

/// Recalcula e aplica a dica contextual a partir do caminho atual no campo.
pub fn apply_dir_hint(ui: &AppWindow) {
    let path = PathBuf::from(ui.get_dir_path().to_string());
    let (text, warning) = dir_hint(&path);
    ui.set_dir_hint(text.into());
    ui.set_dir_hint_warning(warning);
}

/// Dica contextual sobre o diretório-alvo escolhido. Retorna `(texto, é_alerta)`.
///
/// Cobre três casos do feedback de uso real (§ teste Fase 2):
/// 1. caminho vazio → instrução padrão;
/// 2. o usuário apontou uma **subpasta** do OPL (CD/DVD/ART…) em vez da raiz →
///    **alerta**, sugerindo a pasta-pai;
/// 3. a pasta já tem estrutura (CD/ ou DVD/) → nada será recriado; senão, a
///    estrutura será criada ali (só como fallback, não sempre).
pub fn dir_hint(path: &Path) -> (String, bool) {
    if path.as_os_str().is_empty() {
        return (t("hint-empty"), false);
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str())
        && is_opl_subdir_name(name)
    {
        let parent = path
            .parent()
            .map(|p| p.display().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| t("hint-parent-fallback"));
        return (
            t_args(
                "hint-subdir",
                &[("name", name.to_string()), ("parent", parent)],
            ),
            true,
        );
    }

    if path.join("CD").is_dir() || path.join("DVD").is_dir() {
        (t("hint-detected"), false)
    } else {
        (t("hint-will-create"), false)
    }
}
