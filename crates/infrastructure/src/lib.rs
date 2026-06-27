//! `oplhost-infra` — adapters reais que implementam os ports do `core`.
//!
//! Estado atual (scaffold da Fase 1): `RealFs` já é funcional; `SmbBackend`
//! ainda é stub — sua lógica vem da prova de conceito do `spike/` (Fase 0) e
//! será portada quando o teste com PS2 real confirmar a fase.

pub mod real_fs;
pub mod smb_backend;

pub use real_fs::RealFs;
pub use smb_backend::SmbBackend;
