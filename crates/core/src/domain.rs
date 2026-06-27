//! Tipos de domínio compartilhados entre as camadas.

use std::path::PathBuf;

/// Estado observável do servidor de storage (qualquer backend).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Running,
    Stopped,
    /// Estado de falha com mensagem orientando a resolução (UX §8 do CLAUDE.md).
    Error(String),
}

/// Configuração de um share/servidor, independente de backend.
///
/// Genérico de propósito: descreve "onde estão os arquivos do OPL e como
/// expô-los", sem pressupor SMB. Um `UdpbdBackend` futuro reaproveita os mesmos
/// campos relevantes (diretório-alvo, porta) sem precisar de `smb.conf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareConfig {
    /// Raiz escolhida pelo usuário onde vive a estrutura do OPL.
    pub target_dir: PathBuf,
    /// Nome do share exposto (ex.: "PS2SMB").
    pub share_name: String,
    /// Porta do serviço (445 padrão para SMB; configurável).
    pub port: u16,
    /// Usuário dono da pasta, usado em `force user` no backend SMB.
    pub owner_user: String,
}

/// Erro de operação de um `StorageBackend`. Mensagens devem ser descritivas o
/// suficiente para a UI orientar o usuário (porta ocupada, Polkit negado, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Operação root negada/cancelada no prompt do Polkit.
    PrivilegeDenied,
    /// Porta já em uso por outro serviço.
    PortInUse(u16),
    /// Falha genérica com contexto.
    Other(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::PrivilegeDenied => write!(f, "privilégio root negado (Polkit)"),
            BackendError::PortInUse(p) => write!(f, "porta {p} em uso por outro serviço"),
            BackendError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for BackendError {}
