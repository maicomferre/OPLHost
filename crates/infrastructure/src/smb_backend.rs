//! `SmbBackend` — implementação de `StorageBackend` para Samba (SMBv1/NT1).
//!
//! Porta a lógica validada na Fase 0 (ver `plans/fase-0-spike.md`): gera o
//! `opl_share.conf` isolado, injeta o `include` idempotente, reinicia o `smbd` e
//! abre a porta 445 — tudo numa ÚNICA janela de privilégio via o
//! `PrivilegeEscalator`. O rollback desfaz tudo (§0). A montagem dos scripts é
//! pura (`smb_script`); aqui só compomos e delegamos.

use std::process::Command;

use oplhost_core::{BackendError, ServerStatus, ShareConfig, StorageBackend};

use crate::net;
use crate::privilege::{PkexecEscalator, PrivilegeEscalator};
use crate::smb_script::{build_apply_script, build_rollback_script, SmbPaths};

/// Backend Samba, genérico sobre o escalador de privilégio (injetável nos testes).
#[derive(Clone)]
pub struct SmbBackend<E: PrivilegeEscalator = PkexecEscalator> {
    paths: SmbPaths,
    escalator: E,
    /// Config a aplicar/reverter. Necessária para `rollback` fechar a mesma
    /// porta que `apply_config` abriu.
    cfg: ShareConfig,
    /// Senha do usuário Samba para o modo autenticado. **Transitória:** usada só
    /// ao montar o script de apply (`smbpasswd`). `None` no modo guest. Nunca é
    /// serializada nem impressa — ver o `impl Debug` abaixo, que a redige.
    auth_password: Option<String>,
}

/// `Debug` manual: jamais expõe a senha em logs/panics. Tudo o mais é mostrado.
impl<E: PrivilegeEscalator> std::fmt::Debug for SmbBackend<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmbBackend")
            .field("paths", &self.paths)
            .field("cfg", &self.cfg)
            .field(
                "auth_password",
                &self.auth_password.as_ref().map(|_| "<redigida>"),
            )
            .finish_non_exhaustive()
    }
}

impl SmbBackend<PkexecEscalator> {
    /// Backend pronto para produção (Polkit via `pkexec`) com caminhos padrão.
    pub fn new(cfg: ShareConfig) -> Self {
        Self {
            paths: SmbPaths::default(),
            escalator: PkexecEscalator,
            cfg,
            auth_password: None,
        }
    }
}

impl<E: PrivilegeEscalator> SmbBackend<E> {
    /// Constrói com escalador e caminhos explícitos (usado nos testes).
    pub fn with_parts(cfg: ShareConfig, paths: SmbPaths, escalator: E) -> Self {
        Self {
            paths,
            escalator,
            cfg,
            auth_password: None,
        }
    }

    /// Define a senha do usuário Samba para o modo autenticado, consumida só no
    /// `apply_config`. No modo guest passe `None` (ou simplesmente não chame).
    pub fn with_auth_password(mut self, password: Option<String>) -> Self {
        self.auth_password = password;
        self
    }

    /// Consulta `systemctl is-active smbd` (leitura, não precisa de root).
    fn smbd_active(&self) -> bool {
        Command::new("systemctl")
            .args(["is-active", "smbd"])
            .output()
            .map(|o| o.stdout.starts_with(b"active"))
            .unwrap_or(false)
    }
}

impl<E: PrivilegeEscalator> StorageBackend for SmbBackend<E> {
    fn apply_config(&self, cfg: &ShareConfig) -> Result<(), BackendError> {
        // §8: se o smbd está parado mas a porta já tem dono, é outro serviço.
        if !self.smbd_active() && net::tcp_port_listening(cfg.port) {
            return Err(BackendError::PortInUse(cfg.port));
        }
        let script = build_apply_script(&self.paths, cfg, self.auth_password.as_deref());
        self.escalator.run_root_script(&script)
    }

    fn start(&self) -> Result<(), BackendError> {
        self.escalator.run_root_script("systemctl start smbd")
    }

    fn stop(&self) -> Result<(), BackendError> {
        self.escalator.run_root_script("systemctl stop smbd")
    }

    fn status(&self) -> Result<ServerStatus, BackendError> {
        if self.smbd_active() {
            Ok(ServerStatus::Running)
        } else {
            Ok(ServerStatus::Stopped)
        }
    }

    fn rollback(&self) -> Result<(), BackendError> {
        let script = build_rollback_script(&self.paths, &self.cfg);
        self.escalator.run_root_script(&script)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oplhost_core::ShareAuth;
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// Escalador mock: captura o script que receberia, sem rodar nada como root.
    #[derive(Default)]
    struct RecordingEscalator {
        scripts: RefCell<Vec<String>>,
    }

    impl PrivilegeEscalator for RecordingEscalator {
        fn run_root_script(&self, script: &str) -> Result<(), BackendError> {
            self.scripts.borrow_mut().push(script.to_string());
            Ok(())
        }
    }

    fn cfg() -> ShareConfig {
        ShareConfig {
            target_dir: PathBuf::from("/mnt/ps2"),
            share_name: "PS2SMB".to_string(),
            port: 445,
            owner_user: "maicom".to_string(),
            auth: ShareAuth::Guest,
        }
    }

    #[test]
    fn apply_config_entrega_um_unico_script_com_tudo() {
        let esc = RecordingEscalator::default();
        let backend = SmbBackend::with_parts(cfg(), SmbPaths::default(), esc);

        backend.apply_config(&cfg()).unwrap();

        let scripts = backend.escalator.scripts.borrow();
        assert_eq!(scripts.len(), 1, "tudo numa única janela de privilégio");
        let s = &scripts[0];
        assert!(s.contains("opl_share.conf"));
        assert!(s.contains("systemctl restart smbd"));
        assert!(s.contains("ufw allow 445/tcp"));
    }

    #[test]
    fn apply_autenticado_repassa_a_senha_ao_script_numa_unica_janela() {
        let auth = ShareConfig {
            auth: ShareAuth::User {
                username: "maicom".to_string(),
            },
            ..cfg()
        };
        let esc = RecordingEscalator::default();
        let backend = SmbBackend::with_parts(auth.clone(), SmbPaths::default(), esc)
            .with_auth_password(Some("s3nha".to_string()));

        backend.apply_config(&auth).unwrap();

        let scripts = backend.escalator.scripts.borrow();
        assert_eq!(scripts.len(), 1, "auth + share + firewall numa única janela");
        let s = &scripts[0];
        assert!(s.contains("smbpasswd -s -a 'maicom'"));
        assert!(s.contains("'s3nha'"));
        assert!(s.contains("valid users = maicom"));
    }

    #[test]
    fn debug_nao_vaza_a_senha() {
        let esc = RecordingEscalator::default();
        let backend = SmbBackend::with_parts(cfg(), SmbPaths::default(), esc)
            .with_auth_password(Some("supersecreta".to_string()));
        let dump = format!("{backend:?}");
        assert!(!dump.contains("supersecreta"));
        assert!(dump.contains("<redigida>"));
    }

    #[test]
    fn rollback_fecha_a_mesma_porta_e_remove_o_include() {
        let esc = RecordingEscalator::default();
        let backend = SmbBackend::with_parts(cfg(), SmbPaths::default(), esc);

        backend.rollback().unwrap();

        let scripts = backend.escalator.scripts.borrow();
        let s = &scripts[0];
        assert!(s.contains("rm -f /etc/samba/opl_share.conf"));
        assert!(s.contains("ufw delete allow 445/tcp || true"));
    }

    #[test]
    fn start_e_stop_usam_systemctl_via_escalador() {
        let esc = RecordingEscalator::default();
        let backend = SmbBackend::with_parts(cfg(), SmbPaths::default(), esc);

        backend.start().unwrap();
        backend.stop().unwrap();

        let scripts = backend.escalator.scripts.borrow();
        assert_eq!(scripts[0], "systemctl start smbd");
        assert_eq!(scripts[1], "systemctl stop smbd");
    }
}
