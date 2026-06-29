//! Glue entre o estado da UI e o domínio: usuário/diretório, persistência da
//! config de UI (XDG), montagem do `ShareConfig` e leitura do status do servidor.

use std::path::{Path, PathBuf};

use oplhost_core::{
    AppSettings, SETTINGS_VERSION, ServerStatus, SettingsStore, ShareAuth, ShareConfig,
    StorageBackend,
};
use oplhost_infra::{FsSettingsStore, SmbBackend};

use crate::i18n::t;

pub const SHARE_NAME: &str = "PS2SMB";
pub const SMB_PORT: u16 = 445;

/// Usuário dono da pasta (vira `force user` no share). O app roda em user-space.
pub fn current_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "nobody".to_string())
}

/// Lê o estado de UI persistido (XDG). Sem store disponível (sem HOME) → default.
pub fn load_settings() -> AppSettings {
    FsSettingsStore::new().map(|s| s.load()).unwrap_or_default()
}

/// Persiste o estado **não-sensível** da UI (diretório-alvo + toggle de auth) no
/// `config.json` (XDG), best-effort. A senha **nunca** entra aqui — vive no Samba
/// do sistema. Falha de gravação é silenciosa (só loga): persistência é
/// conveniência e não pode atrapalhar a operação principal.
pub fn save_settings(last_target_dir: Option<PathBuf>, auth_required: bool) {
    let Some(store) = FsSettingsStore::new() else {
        return;
    };
    let settings = AppSettings {
        version: SETTINGS_VERSION,
        last_target_dir,
        auth_required,
        auth_username: Some(current_user()),
    };
    if let Err(e) = store.save(&settings) {
        eprintln!("[oplhost] não foi possível salvar config.json: {e}");
    }
}

pub fn share_config(target: &Path, auth: ShareAuth) -> ShareConfig {
    ShareConfig {
        target_dir: target.to_path_buf(),
        share_name: SHARE_NAME.to_string(),
        port: SMB_PORT,
        owner_user: current_user(),
        auth,
    }
}

/// Modo de acesso a partir do estado da UI: autenticado (usuário = dono da
/// pasta) quando o toggle está ligado, senão guest (padrão).
pub fn auth_mode(enabled: bool) -> ShareAuth {
    if enabled {
        ShareAuth::User {
            username: current_user(),
        }
    } else {
        ShareAuth::Guest
    }
}

/// Estado atual do servidor para a UI: `(texto, ativo)`. "Ativo" = a config do
/// OPL está aplicada (share isolado + include), derivado do backend — não do
/// daemon global (decisão 2026-06-27). Sem root (leitura de arquivos).
pub fn current_status() -> (String, bool) {
    let backend = SmbBackend::new(share_config(Path::new("/"), ShareAuth::Guest));
    let active = matches!(backend.status(), Ok(ServerStatus::Running));
    let text = if active {
        t("status-active")
    } else {
        t("status-inactive")
    };
    (text, active)
}
