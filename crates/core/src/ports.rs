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
/// Modelo "**aplicar/remover configuração**": o backend gerencia só a config
/// que faz o catálogo do OPL ser servido (no SMB: o share isolado + include +
/// firewall, com um *reload* do daemon), sem controlar o ciclo de vida de um
/// daemon do sistema que pode atender a outros usos. Por isso o contrato é
/// `apply_config` / `status` / `rollback` — não há `start`/`stop` de processo.
///
/// **Decisão (2026-06-27):** as operações antigas `start`/`stop` (que faziam
/// `systemctl start/stop smbd` no daemon global) foram removidas — paravam o
/// Samba do sistema inteiro, violando o isolamento (§0). Controle de ciclo de
/// vida de um **processo dedicado** só fará sentido no futuro `UdpbdBackend`
/// (§7.1), que supervisiona seu próprio servidor; a abstração será revisitada
/// ali, com os dois casos concretos na mão (CLAUDE.md §7.1). Até lá, este Trait
/// diverge da lista de §3 do CLAUDE.md de propósito — ver `plans/`.
pub trait StorageBackend {
    /// Gera/aplica a configuração necessária para servir o diretório-alvo.
    fn apply_config(&self, cfg: &ShareConfig) -> Result<(), BackendError>;
    /// Estado atual: a configuração do OPL está aplicada (servindo) ou não.
    fn status(&self) -> Result<ServerStatus, BackendError>;
    /// Desfaz toda a configuração aplicada (rollback completo).
    fn rollback(&self) -> Result<(), BackendError>;
}
