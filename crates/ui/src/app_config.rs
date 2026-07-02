//! Glue entre o estado da UI e o domínio: usuário/diretório, persistência da
//! config de UI (XDG), montagem do `ShareConfig` e leitura do status do servidor.

use std::path::{Path, PathBuf};

use oplhost_core::{
    AppSettings, BackendKind, SETTINGS_VERSION, ServerStatus, SettingsStore, ShareAuth,
    ShareConfig, StorageBackend, UdpbdConfig,
};
use oplhost_infra::{FsSettingsStore, UdpbdBackend, opl_share_status};

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
    // Preserva o que não é passado nesta chamada (backend escolhido + device do
    // UDPBD): estas rotas só mexem em diretório/auth do SMB, não podem zerar a
    // seleção de backend feita nos Settings.
    let prev = store.load();
    let settings = AppSettings {
        version: SETTINGS_VERSION,
        last_target_dir,
        auth_required,
        auth_username: Some(current_user()),
        backend_kind: prev.backend_kind,
        udpbd_device: prev.udpbd_device,
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

/// Persiste a escolha de backend + o device do UDPBD, preservando os demais
/// campos (diretório/auth do SMB). Best-effort, como `save_settings`.
pub fn save_backend_selection(backend_udpbd: bool, udpbd_device: Option<PathBuf>) {
    let Some(store) = FsSettingsStore::new() else {
        return;
    };
    let prev = store.load();
    let settings = AppSettings {
        version: SETTINGS_VERSION,
        last_target_dir: prev.last_target_dir,
        auth_required: prev.auth_required,
        auth_username: prev.auth_username.or_else(|| Some(current_user())),
        backend_kind: if backend_udpbd {
            BackendKind::Udpbd
        } else {
            BackendKind::Smb
        },
        udpbd_device,
    };
    if let Err(e) = store.save(&settings) {
        eprintln!("[oplhost] não foi possível salvar config.json: {e}");
    }
}

/// Estado atual do servidor para a UI: `(texto, ativo)`. Backend-aware: no SMB,
/// "ativo" = a config do OPL está aplicada (share isolado + include), lida dos
/// caminhos padrão do Samba sem root; no UDPBD, = a unit do `udpbd-server` está
/// ativa (`systemctl is-active`). Lê o backend escolhido do `config.json`.
pub fn current_status() -> (String, bool) {
    let active = matches!(current_server_status(), ServerStatus::Running);
    let text = if active {
        t("status-active")
    } else {
        t("status-inactive")
    };
    (text, active)
}

/// Status bruto do backend persistido. UDPBD sem device escolhido → `Stopped`.
fn current_server_status() -> ServerStatus {
    let s = load_settings();
    match s.backend_kind {
        BackendKind::Smb => opl_share_status(),
        BackendKind::Udpbd => match s.udpbd_device {
            Some(device) => UdpbdBackend::new(UdpbdConfig { device })
                .status()
                .unwrap_or(ServerStatus::Stopped),
            None => ServerStatus::Stopped,
        },
    }
}
