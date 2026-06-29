//! `oplhost-core` — regras de negócio agnósticas a I/O, rede e UI.
//!
//! Aqui ficam os tipos de domínio, os Traits (ports) que a `infrastructure`
//! implementa e a lógica pura (ex.: estruturação de pastas do OPL). Nada neste
//! crate toca disco, rede ou processos diretamente — isso o mantém testável
//! com mocks e independente da troca de backend (SMB hoje, UDPBD amanhã).

pub mod catalog;
pub mod domain;
pub mod game_id;
pub mod iso9660;
pub mod meta;
pub mod opl_layout;
pub mod ports;
pub mod settings;

pub use catalog::{CatalogSummary, GameEntry, Media, is_game_image_name, summarize};
pub use domain::{BackendError, ServerStatus, ShareAuth, ShareConfig};
pub use game_id::{GameId, derive_title, parse_boot2_game_id};
pub use meta::{GameMeta, MediaKind, MetaError, MetaStore, OplMeta};
pub use opl_layout::{create_opl_layout, is_opl_subdir_name};
pub use ports::{Fs, StorageBackend};
pub use settings::{AppSettings, SETTINGS_VERSION, SettingsError, SettingsStore};
