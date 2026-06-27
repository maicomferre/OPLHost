//! Ports (Traits) que a `infrastructure` implementa. A inversão de dependência
//! acontece aqui: o `core` define o contrato; os adapters reais o satisfazem.

use std::path::Path;

use crate::domain::{BackendError, ServerStatus, ShareConfig};

/// Port de filesystem. Existe para o `core` permanecer agnóstico a I/O: a
/// lógica de estruturação de pastas depende deste Trait, não de `std::fs`, e
/// nos testes é substituído por um mock.
pub trait Fs {
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
}

/// Port do servidor de storage do OPL. **Genérico, não casado com SMB.**
///
/// Primeira implementação: `SmbBackend`. Um futuro `UdpbdBackend` (§7.1) deve
/// caber neste mesmo contrato sem refatoração dolorosa — por isso as operações
/// são descritas em termos de "aplicar config / iniciar / parar / status /
/// reverter", e não em termos de `smb.conf`.
pub trait StorageBackend {
    /// Gera/aplica a configuração necessária para servir o diretório-alvo.
    fn apply_config(&self, cfg: &ShareConfig) -> Result<(), BackendError>;
    /// Inicia o serviço.
    fn start(&self) -> Result<(), BackendError>;
    /// Para o serviço.
    fn stop(&self) -> Result<(), BackendError>;
    /// Estado atual do serviço.
    fn status(&self) -> Result<ServerStatus, BackendError>;
    /// Desfaz toda a configuração aplicada (rollback completo).
    fn rollback(&self) -> Result<(), BackendError>;
}
