//! `FsSettingsStore` — implementação de `SettingsStore` sobre um arquivo JSON em
//! `$XDG_CONFIG_HOME/oplhost/config.json` (fallback `~/.config/oplhost/`).
//!
//! Espelha o `JsonMetaStore`: a tolerância a erro fica aqui, não no `core`.
//! `load` nunca falha — arquivo ausente OU corrompido vira `AppSettings::default()`
//! (§6: a config é conveniência, jamais fonte de verdade). Só `save` reporta erro,
//! e a UI o trata como best-effort.
//!
//! XDG resolvido por variáveis de ambiente (sem crate `directories`/`dirs`):
//! `$XDG_CONFIG_HOME` se setada e absoluta, senão `$HOME/.config` (Base Directory
//! Specification). Evita dependência especulativa (CLAUDE.md §3/§12).

use std::path::{Path, PathBuf};

use oplhost_core::{AppSettings, SettingsError, SettingsStore};

/// Nome fixo do arquivo de config sob o diretório da app.
pub const CONFIG_FILE_NAME: &str = "config.json";
/// Subdiretório da app dentro do config dir do XDG.
pub const APP_DIR_NAME: &str = "oplhost";

/// Persiste a config de UI no diretório de config do usuário (XDG).
#[derive(Debug, Clone)]
pub struct FsSettingsStore {
    file_path: PathBuf,
}

impl FsSettingsStore {
    /// Cria o store apontando para `<config_dir>/oplhost/config.json`, com
    /// `<config_dir>` resolvido pelo XDG. Retorna `None` se nem `$XDG_CONFIG_HOME`
    /// nem `$HOME` puderem ser resolvidos (ambiente sem home — raríssimo); aí a
    /// UI simplesmente roda sem persistência.
    pub fn new() -> Option<Self> {
        let dir = config_base_dir()?.join(APP_DIR_NAME);
        Some(Self {
            file_path: dir.join(CONFIG_FILE_NAME),
        })
    }

    /// Constrói um store com caminho explícito (usado nos testes).
    pub fn with_path(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    /// Caminho do arquivo de config gerenciado.
    pub fn path(&self) -> &Path {
        &self.file_path
    }
}

/// Diretório base de config segundo o XDG: `$XDG_CONFIG_HOME` quando setado e
/// absoluto (a spec manda ignorar caminhos relativos), senão `$HOME/.config`.
fn config_base_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg);
        if p.is_absolute() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config"))
}

impl SettingsStore for FsSettingsStore {
    fn load(&self) -> AppSettings {
        // Ausente ou ilegível → default. Corrompido → default. Nunca derruba (§6).
        match std::fs::read_to_string(&self.file_path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => AppSettings::default(),
        }
    }

    fn save(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SettingsError::Io(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| SettingsError::Serialize(e.to_string()))?;
        std::fs::write(&self.file_path, json).map_err(|e| SettingsError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::unique_path;
    use std::path::PathBuf;

    /// Arquivo temporário único POR CHAMADA — os testes rodam em paralelo e não
    /// podem disputar o mesmo caminho.
    fn temp_config_path() -> PathBuf {
        unique_path("settings").join(CONFIG_FILE_NAME)
    }

    #[test]
    fn load_de_arquivo_inexistente_e_default() {
        let store = FsSettingsStore::with_path(temp_config_path());
        let _ = std::fs::remove_file(store.path());
        assert_eq!(store.load(), AppSettings::default());
    }

    #[test]
    fn save_cria_o_diretorio_e_load_preserva() {
        let store = FsSettingsStore::with_path(temp_config_path());
        let settings = AppSettings {
            last_target_dir: Some(PathBuf::from("/mnt/ps2")),
            auth_required: true,
            auth_username: Some("maicom".to_string()),
            ..Default::default()
        };
        store.save(&settings).unwrap();
        assert!(store.path().exists());
        assert_eq!(store.load(), settings);
        let _ = std::fs::remove_file(store.path());
        let _ = std::fs::remove_dir(store.path().parent().unwrap());
    }

    #[test]
    fn json_corrompido_cai_em_default_sem_panico() {
        let store = FsSettingsStore::with_path(temp_config_path());
        std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        std::fs::write(store.path(), "{ isto não é json }").unwrap();
        assert_eq!(store.load(), AppSettings::default());
        let _ = std::fs::remove_file(store.path());
        let _ = std::fs::remove_dir(store.path().parent().unwrap());
    }

    #[test]
    fn config_salva_jamais_contem_senha_no_disco() {
        let store = FsSettingsStore::with_path(temp_config_path());
        store
            .save(&AppSettings {
                auth_required: true,
                auth_username: Some("maicom".to_string()),
                ..Default::default()
            })
            .unwrap();
        let raw = std::fs::read_to_string(store.path())
            .unwrap()
            .to_lowercase();
        assert!(!raw.contains("password") && !raw.contains("senha") && !raw.contains("passwd"));
        let _ = std::fs::remove_file(store.path());
        let _ = std::fs::remove_dir(store.path().parent().unwrap());
    }
}
