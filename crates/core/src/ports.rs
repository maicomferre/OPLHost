//! Ports (Traits) que a `infrastructure` implementa. A inversão de dependência
//! acontece aqui: o `core` define o contrato; os adapters reais o satisfazem.

use std::path::Path;

use crate::domain::{BackendError, ServerStatus};

/// Port de filesystem. Existe para o `core` permanecer agnóstico a I/O: a
/// lógica de estruturação de pastas depende deste Trait, não de `std::fs`, e
/// nos testes é substituído por um mock.
pub trait Fs {
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
}

/// Port do servidor de storage do OPL. **Genérico, não casado com SMB.**
///
/// Contrato **verbal** de 3 operações que servem aos dois modelos concretos
/// (revisitado na fase UDPBD, `plans/fase-3-udpbd-backend.md`, com SMB como 2º
/// caso concreto — CLAUDE.md §7.1):
///
/// - **`apply`** — passa a servir o catálogo do OPL. No `SmbBackend` é
///   declarativo: gera o share isolado + include + firewall e dá *reload* no
///   `smbd` (nunca start/stop do daemon **global**, §0). No `UdpbdBackend` é
///   supervisão: sobe o processo dedicado do servidor (via unit do systemd) +
///   firewall UDP.
/// - **`status`** — o catálogo está sendo servido? Derivado do que cada backend
///   considera "servindo" (SMB: config aplicada; UDPBD: processo vivo) — nunca do
///   estado de um daemon global.
/// - **`rollback`** — deixa de servir e desfaz o que `apply` fez.
///
/// **Sem `ShareConfig` no contrato (decisão 2026-07-01):** cada backend carrega a
/// **própria** config na construção (o SMB usa `ShareConfig`; o UDPBD, um
/// `UdpbdConfig`). Assim o trait não conhece campos SMB-flavored (`share_name`,
/// `auth`…). As antigas `start`/`stop` (que mexiam no `smbd` global) seguem fora:
/// o processo do UDPBD é **dedicado** ao app, não um daemon compartilhado.
pub trait StorageBackend {
    /// Passa a servir o catálogo do OPL (aplica config e/ou sobe o servidor).
    fn apply(&self) -> Result<(), BackendError>;
    /// Estado atual: o catálogo do OPL está sendo servido ou não.
    fn status(&self) -> Result<ServerStatus, BackendError>;
    /// Deixa de servir e desfaz tudo que `apply` fez (rollback completo).
    fn rollback(&self) -> Result<(), BackendError>;
}
