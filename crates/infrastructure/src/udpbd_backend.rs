//! `UdpbdBackend` — implementação de `StorageBackend` para o UDPBD (BDM).
//!
//! **Supervisiona um servidor existente** (`udpbd-server`), NUNCA reimplementa o
//! protocolo (§7.1). O servidor é um processo bloqueante sem daemon próprio, então
//! o app o carrega numa **unit transiente do systemd** (`systemd-run`): sobrevive
//! ao fechar a janela e dá `status` via `systemctl is-active`.
//!
//! **Escopo condicional ao alvo (validado na fonte):** a porta UDP 48573 é >1024
//! (sem root); o que exige privilégio é o acesso r/w ao device. Um raw `/dev/sdX`
//! → unit de **sistema** na janela Polkit; um arquivo-imagem do usuário → unit
//! `--user`, sem senha. A montagem dos scripts é pura (`udpbd_script`); aqui só
//! decidimos o escopo, compomos e delegamos ao runner certo.

use std::path::Path;
use std::process::Command;

use oplhost_core::{BackendError, ServerStatus, StorageBackend, UdpbdConfig};

use crate::privilege::{PkexecEscalator, PrivilegeEscalator};
use crate::udpbd_script::{UDPBD_UNIT, build_apply_script, build_rollback_script};

/// Nome do binário do servidor procurado no PATH quando nenhum caminho explícito
/// é dado. Configurável para apontar um build que case com a versão do OPL.
pub const DEFAULT_SERVER_BIN: &str = "udpbd-server";

/// Executa um script de shell **como o usuário** (sem elevação). Contraparte do
/// [`PrivilegeEscalator`] para o caminho `--user` (imagem-arquivo). Trait para o
/// backend ser testável com um runner mock que captura o script.
pub trait UserShell {
    fn run_user_script(&self, script: &str) -> Result<(), BackendError>;
}

/// Runner real: roda `bash -c <script>` como o usuário atual.
#[derive(Debug, Default, Clone, Copy)]
pub struct BashUserShell;

impl UserShell for BashUserShell {
    fn run_user_script(&self, script: &str) -> Result<(), BackendError> {
        let full = format!("set -euo pipefail\n{script}\n");
        let status = Command::new("bash")
            .arg("-c")
            .arg(&full)
            .status()
            .map_err(|e| BackendError::Other(format!("falha ao invocar bash: {e}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(BackendError::Other(format!(
                "udpbd-server (systemd --user) falhou (status {:?})",
                status.code()
            )))
        }
    }
}

/// Decide se o alvo exige root: um **block device** (`/dev/…`) sim; um
/// arquivo-imagem do usuário, não. Base do escopo systemd (sistema vs `--user`).
pub fn needs_root(device: &Path) -> bool {
    device.starts_with("/dev/")
}

/// Backend UDPBD, genérico sobre o escalador root e o shell de usuário (ambos
/// injetáveis nos testes).
#[derive(Debug, Clone)]
pub struct UdpbdBackend<E: PrivilegeEscalator = PkexecEscalator, R: UserShell = BashUserShell> {
    cfg: UdpbdConfig,
    server_bin: String,
    escalator: E,
    shell: R,
}

impl UdpbdBackend<PkexecEscalator, BashUserShell> {
    /// Backend pronto para produção: Polkit (`pkexec`) para o caso raw device,
    /// bash `--user` para o caso imagem, e `udpbd-server` resolvido no PATH.
    pub fn new(cfg: UdpbdConfig) -> Self {
        Self {
            cfg,
            server_bin: DEFAULT_SERVER_BIN.to_string(),
            escalator: PkexecEscalator,
            shell: BashUserShell,
        }
    }
}

impl<E: PrivilegeEscalator, R: UserShell> UdpbdBackend<E, R> {
    /// Constrói com runners e binário explícitos (usado nos testes).
    pub fn with_parts(
        cfg: UdpbdConfig,
        server_bin: impl Into<String>,
        escalator: E,
        shell: R,
    ) -> Self {
        Self {
            cfg,
            server_bin: server_bin.into(),
            escalator,
            shell,
        }
    }

    /// Aponta um binário de `udpbd-server` específico (ex.: um build que case com
    /// a versão do OPL do console).
    pub fn with_server_bin(mut self, bin: impl Into<String>) -> Self {
        self.server_bin = bin.into();
        self
    }

    /// O binário do `udpbd-server` está disponível? A UI usa para avisar antes de
    /// tentar (§8), já que a V1 não instala o servidor. Caminho absoluto → checa
    /// o arquivo; nome simples → procura no PATH via `command -v`.
    pub fn server_available(&self) -> bool {
        if self.server_bin.contains('/') {
            return Path::new(&self.server_bin).is_file();
        }
        Command::new("bash")
            .arg("-c")
            .arg(format!("command -v {} >/dev/null 2>&1", self.server_bin))
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// `true` quando o alvo é um arquivo-imagem (roda `--user`, sem root).
    fn user_scope(&self) -> bool {
        !needs_root(&self.cfg.device)
    }
}

impl<E: PrivilegeEscalator, R: UserShell> StorageBackend for UdpbdBackend<E, R> {
    fn apply(&self) -> Result<(), BackendError> {
        let user_scope = self.user_scope();
        let script = build_apply_script(&self.cfg, &self.server_bin, user_scope);
        if user_scope {
            self.shell.run_user_script(&script)
        } else {
            self.escalator.run_root_script(&script)
        }
    }

    fn status(&self) -> Result<ServerStatus, BackendError> {
        // "Servindo" = a unit do servidor está ativa. Leitura tolerante: qualquer
        // falha (systemctl ausente, unit inexistente) → Stopped, nunca erro/panic.
        let user_scope = self.user_scope();
        let scope = if user_scope { "--user " } else { "" };
        let out = Command::new("bash")
            .arg("-c")
            .arg(format!("systemctl {scope}is-active {UDPBD_UNIT}"))
            .output();
        let active = out
            .map(|o| o.stdout.starts_with(b"active"))
            .unwrap_or(false);
        Ok(if active {
            ServerStatus::Running
        } else {
            ServerStatus::Stopped
        })
    }

    fn rollback(&self) -> Result<(), BackendError> {
        let user_scope = self.user_scope();
        let script = build_rollback_script(user_scope);
        if user_scope {
            self.shell.run_user_script(&script)
        } else {
            self.escalator.run_root_script(&script)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// Escalador root mock: captura o script sem rodar nada como root.
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

    /// Shell de usuário mock: captura o script sem rodar bash.
    #[derive(Default)]
    struct RecordingShell {
        scripts: RefCell<Vec<String>>,
    }
    impl UserShell for RecordingShell {
        fn run_user_script(&self, script: &str) -> Result<(), BackendError> {
            self.scripts.borrow_mut().push(script.to_string());
            Ok(())
        }
    }

    fn backend_for(device: &str) -> UdpbdBackend<RecordingEscalator, RecordingShell> {
        UdpbdBackend::with_parts(
            UdpbdConfig {
                device: PathBuf::from(device),
            },
            "/usr/bin/udpbd-server",
            RecordingEscalator::default(),
            RecordingShell::default(),
        )
    }

    #[test]
    fn raw_device_usa_root_e_abre_firewall_numa_janela() {
        let b = backend_for("/dev/sdb1");
        b.apply().unwrap();
        // foi pela via root (Polkit), não pelo shell de usuário
        assert_eq!(b.escalator.scripts.borrow().len(), 1);
        assert_eq!(b.shell.scripts.borrow().len(), 0);
        let s = &b.escalator.scripts.borrow()[0];
        assert!(s.contains("systemd-run --unit=oplhost-udpbd"));
        assert!(s.contains("'/dev/sdb1'"));
        assert!(s.contains("48573/udp"));
    }

    #[test]
    fn imagem_usa_user_scope_sem_root_nem_firewall() {
        let b = backend_for("/home/maicom/ps2.img");
        b.apply().unwrap();
        // foi pela via de usuário, sem Polkit
        assert_eq!(b.shell.scripts.borrow().len(), 1);
        assert_eq!(b.escalator.scripts.borrow().len(), 0);
        let s = &b.shell.scripts.borrow()[0];
        assert!(s.contains("systemd-run --user --unit=oplhost-udpbd"));
        assert!(!s.contains("ufw"));
    }

    #[test]
    fn rollback_raw_device_para_a_unit_e_fecha_a_porta_como_root() {
        let b = backend_for("/dev/sdb1");
        b.rollback().unwrap();
        let s = &b.escalator.scripts.borrow()[0];
        assert!(s.contains("systemctl stop oplhost-udpbd"));
        assert!(s.contains("ufw delete allow 48573/udp || true"));
    }

    #[test]
    fn rollback_imagem_para_a_unit_user_sem_root() {
        let b = backend_for("/home/maicom/ps2.img");
        b.rollback().unwrap();
        let s = &b.shell.scripts.borrow()[0];
        assert!(s.contains("systemctl --user stop oplhost-udpbd"));
        assert_eq!(b.escalator.scripts.borrow().len(), 0);
    }

    #[test]
    fn needs_root_so_para_dev() {
        assert!(needs_root(Path::new("/dev/sda")));
        assert!(!needs_root(Path::new("/home/user/disk.img")));
        assert!(!needs_root(Path::new("/mnt/ext/ps2.img")));
    }

    #[test]
    fn server_available_checa_arquivo_absoluto() {
        let b = backend_for("/dev/sdb1"); // server_bin = /usr/bin/udpbd-server
        // caminho absoluto inexistente → indisponível (sem tocar no PATH)
        assert!(!b.server_available());
    }
}
