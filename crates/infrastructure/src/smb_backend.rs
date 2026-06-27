//! `SmbBackend` — implementação de `StorageBackend` para Samba (SMBv1/NT1).
//!
//! STUB do scaffold da Fase 1. A lógica real (gerar `opl_share.conf`, injetar o
//! `include` de forma idempotente, reiniciar `smbd` e abrir a porta 445 numa
//! única janela Polkit) está provada no `spike/` da Fase 0 e será portada para
//! cá quando a fase for aprovada com PS2 real. Caminhos de referência:
//!   /etc/samba/opl_share.conf  +  include no /etc/samba/smb.conf

use oplhost_core::{BackendError, ServerStatus, ShareConfig, StorageBackend};

/// Backend Samba. Mantém o caminho do share isolado para gerar/reverter config.
#[derive(Debug, Clone)]
pub struct SmbBackend {
    /// Arquivo de config isolado gerenciado pelo app (nunca o smb.conf global).
    pub share_conf_path: String,
}

impl Default for SmbBackend {
    fn default() -> Self {
        Self {
            share_conf_path: "/etc/samba/opl_share.conf".to_string(),
        }
    }
}

impl StorageBackend for SmbBackend {
    fn apply_config(&self, _cfg: &ShareConfig) -> Result<(), BackendError> {
        // TODO(fase-1): portar do spike — gerar opl_share.conf (SMBv1), injetar
        // include idempotente, reiniciar smbd e abrir porta 445 via 1 pkexec.
        todo!("portar lógica validada no spike da Fase 0")
    }

    fn start(&self) -> Result<(), BackendError> {
        todo!("systemctl start smbd via PrivilegeEscalator")
    }

    fn stop(&self) -> Result<(), BackendError> {
        todo!("systemctl stop smbd via PrivilegeEscalator")
    }

    fn status(&self) -> Result<ServerStatus, BackendError> {
        todo!("consultar estado do smbd")
    }

    fn rollback(&self) -> Result<(), BackendError> {
        // TODO(fase-1): remover opl_share.conf + linha de include, reiniciar
        // smbd e remover a regra de firewall (numa única janela Polkit).
        todo!("portar rollback validado no spike da Fase 0")
    }
}
