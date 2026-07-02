//! `oplhost-infra` — adapters reais que implementam os ports do `core`.
//!
//! Estado da Fase 1: `RealFs` e `SmbBackend` funcionais (lógica portada do
//! spike validado na Fase 0); `JsonMetaStore` persiste o `opl_meta.json`;
//! `PkexecEscalator` agrupa as operações root numa janela Polkit; o
//! `FirewallManager` abre/fecha a porta. Tudo depende do `core`; o `core`
//! nunca depende daqui.

pub mod art;
pub mod dialog;
pub mod firewall;
pub mod fs_game_info_store;
pub mod fs_settings_store;
pub mod iso;
pub mod meta_store;
pub mod net;
pub mod privilege;
pub mod real_fs;
pub mod scan;
pub mod smb_backend;
pub mod smb_script;
pub mod udpbd_backend;
pub mod udpbd_script;

#[cfg(test)]
mod test_util;

pub use art::{ArtError, ArtProvider, ArtType, FetchOutcome, HttpGet, UreqClient};
pub use firewall::{FirewallManager, Protocol};
pub use fs_game_info_store::FsGameInfoStore;
pub use fs_settings_store::FsSettingsStore;
pub use meta_store::JsonMetaStore;
pub use privilege::{PkexecEscalator, PrivilegeEscalator};
pub use real_fs::RealFs;
pub use smb_backend::{SmbBackend, opl_share_status};
pub use smb_script::{SmbPaths, build_apply_script, build_rollback_script, build_smb_conf};
pub use udpbd_backend::{BashUserShell, DEFAULT_SERVER_BIN, UdpbdBackend, UserShell, needs_root};
