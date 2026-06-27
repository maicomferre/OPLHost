//! `PrivilegeEscalator` — executa operações root numa ÚNICA janela de privilégio.
//!
//! Regra inegociável do projeto (§5): agrupar tudo que precisa de root num só
//! script e dispará-lo via um único `pkexec`, para o usuário digitar a senha uma
//! vez. O backend monta o corpo do script; este adapter só o executa como root.
//! Definido como Trait para o `SmbBackend` ser testável com um escalador mock.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use oplhost_core::BackendError;

/// Contrato de elevação de privilégio. A implementação real usa Polkit/`pkexec`;
/// os testes injetam um mock que captura o script sem rodar nada.
pub trait PrivilegeEscalator {
    /// Roda `script` (corpo de um `/bin/bash`) como root, numa janela só.
    fn run_root_script(&self, script: &str) -> Result<(), BackendError>;
}

/// Implementação via `pkexec` (prompt nativo do Polkit). Grava o script num
/// arquivo temporário e o executa com `pkexec /bin/bash <arquivo>`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PkexecEscalator;

impl PrivilegeEscalator for PkexecEscalator {
    fn run_root_script(&self, script: &str) -> Result<(), BackendError> {
        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!("oplhost-root-{}.sh", std::process::id()));

        let full = format!("#!/bin/bash\nset -euo pipefail\n{script}\n");
        fs::write(&path, full)
            .map_err(|e| BackendError::Other(format!("falha ao escrever script root: {e}")))?;

        let status = Command::new("pkexec")
            .arg("/bin/bash")
            .arg(&path)
            .status();

        let _ = fs::remove_file(&path);

        let status = status
            .map_err(|e| BackendError::Other(format!("falha ao invocar pkexec: {e}")))?;

        if status.success() {
            return Ok(());
        }
        // pkexec usa 126 (diálogo cancelado/dismiss) e 127 (não autorizado) para
        // recusa do usuário — mapeados para PrivilegeDenied (§8: sem crash).
        match status.code() {
            Some(126) | Some(127) => Err(BackendError::PrivilegeDenied),
            Some(c) => Err(BackendError::Other(format!(
                "operação root falhou (status {c})"
            ))),
            None => Err(BackendError::Other(
                "operação root terminada por sinal".to_string(),
            )),
        }
    }
}
