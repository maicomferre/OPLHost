//! `oplhost-core` — regras de negócio agnósticas a I/O, rede e UI.
//!
//! Aqui ficam os tipos de domínio, os Traits (ports) que a `infrastructure`
//! implementa e a lógica pura (ex.: estruturação de pastas do OPL). Nada neste
//! crate toca disco, rede ou processos diretamente — isso o mantém testável
//! com mocks e independente da troca de backend (SMB hoje, UDPBD amanhã).

pub mod domain;
pub mod opl_layout;
pub mod ports;

pub use domain::{BackendError, ServerStatus, ShareConfig};
pub use ports::{Fs, StorageBackend};
