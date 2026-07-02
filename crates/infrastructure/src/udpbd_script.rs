//! Construtores PUROS dos scripts do backend UDPBD.
//!
//! Espelha o `smb_script.rs`: gerar os corpos de shell (subir/parar o servidor +
//! firewall) fica em funções puras, sem I/O nem root, verificadas à exaustão
//! pelos testes. O `UdpbdBackend` só compõe estes pedaços e os entrega a um
//! runner (systemd `--user`) ou ao `PrivilegeEscalator` (unit de sistema).
//!
//! **Modelo (validado na fonte, `plans/fase-3-udpbd-backend.md`):** o
//! `udpbd-server <device>` é um processo **bloqueante** que expõe um block
//! device/imagem via **UDP 48573**. Como não tem daemon próprio, o app o
//! supervisiona por uma **unit transiente do systemd** (`systemd-run`), que
//! sobrevive ao fechar a janela e dá `status` via `systemctl is-active`.

use oplhost_core::{UDPBD_PORT, UdpbdConfig};

use crate::firewall::{FirewallManager, Protocol};

/// Nome da unit transiente do systemd que carrega o `udpbd-server`. Fixo para o
/// `rollback`/`status` referenciarem exatamente a mesma unit que o `apply` criou.
pub const UDPBD_UNIT: &str = "oplhost-udpbd";

/// Aspas simples seguras para shell (idêntico ao `smb_script`): envolve em `'…'`
/// e escapa aspas simples internas. O caminho do device é interpolado no script.
fn sh_squote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Prefixo do `systemd-run`: `--user` (imagem/arquivo do usuário, sem root) ou de
/// sistema (raw `/dev/sdX`, na janela Polkit). O nome da unit é o mesmo nos dois.
fn systemd_run(user_scope: bool) -> String {
    let scope = if user_scope { " --user" } else { "" };
    format!("systemd-run{scope} --unit={UDPBD_UNIT} --collect")
}

/// Prefixo do `systemctl` correspondente ao escopo do `systemd-run`.
fn systemctl(user_scope: bool) -> String {
    let scope = if user_scope { " --user" } else { "" };
    format!("systemctl{scope}")
}

/// Corpo do script de APPLY: sobe o `udpbd-server <device>` como unit transiente
/// e abre a porta UDP 48573 no firewall — na mesma janela quando de sistema.
///
/// `server_bin` é o caminho do binário do `udpbd-server` (resolvido pelo backend;
/// permite apontar para um build específico que case com a versão do OPL). Falha
/// de "unit já existe" é evitada com `--collect` + reset prévio.
pub fn build_apply_script(cfg: &UdpbdConfig, server_bin: &str, user_scope: bool) -> String {
    let run = systemd_run(user_scope);
    let ctl = systemctl(user_scope);
    let device = sh_squote(&cfg.device.to_string_lossy());
    let bin = sh_squote(server_bin);
    // Firewall: quando de sistema, entra no mesmo script root; no escopo --user
    // não há como abrir a porta (sem root) — o backend trata isso separadamente.
    let firewall = if user_scope {
        String::new()
    } else {
        format!(
            "\n\n{}",
            FirewallManager.open_fragment(UDPBD_PORT, Protocol::Udp)
        )
    };
    format!(
        "{ctl} reset-failed {UDPBD_UNIT} 2>/dev/null || true\n\
         {run} {bin} {device}{firewall}\n"
    )
}

/// Corpo do script de ROLLBACK: para a unit e fecha a porta. Tolerante a unit
/// ausente (`|| true`): o rollback nunca falha por isso.
pub fn build_rollback_script(user_scope: bool) -> String {
    let ctl = systemctl(user_scope);
    let firewall = if user_scope {
        String::new()
    } else {
        format!(
            "\n\n{}",
            FirewallManager.close_fragment(UDPBD_PORT, Protocol::Udp)
        )
    };
    format!(
        "{ctl} stop {UDPBD_UNIT} 2>/dev/null || true\n\
         {ctl} reset-failed {UDPBD_UNIT} 2>/dev/null || true{firewall}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> UdpbdConfig {
        UdpbdConfig {
            device: PathBuf::from("/dev/sdb1"),
        }
    }

    #[test]
    fn apply_sobe_a_unit_com_o_device_e_a_porta_udp() {
        let s = build_apply_script(&cfg(), "/usr/bin/udpbd-server", false);
        assert!(s.contains("systemd-run --unit=oplhost-udpbd --collect"));
        assert!(s.contains("'/usr/bin/udpbd-server' '/dev/sdb1'"));
        // firewall UDP 48573 na mesma janela (escopo de sistema)
        assert!(s.contains("48573/udp"));
        // reset prévio evita colisão de unit já existente
        assert!(s.contains("reset-failed oplhost-udpbd"));
    }

    #[test]
    fn apply_user_scope_usa_systemd_run_user_e_nao_mexe_no_firewall() {
        let s = build_apply_script(&cfg(), "udpbd-server", true);
        assert!(s.contains("systemd-run --user --unit=oplhost-udpbd"));
        // sem root não há como abrir a porta aqui — fica fora do script --user
        assert!(!s.contains("ufw"));
        assert!(!s.contains("iptables"));
    }

    #[test]
    fn device_com_aspas_e_escapado_para_o_shell() {
        let c = UdpbdConfig {
            device: PathBuf::from("/mnt/it's/ps2.img"),
        };
        let s = build_apply_script(&c, "udpbd-server", true);
        assert!(s.contains(r"'/mnt/it'\''s/ps2.img'"));
    }

    #[test]
    fn rollback_para_a_unit_e_fecha_a_porta() {
        let s = build_rollback_script(false);
        assert!(s.contains("systemctl stop oplhost-udpbd"));
        assert!(s.contains("ufw delete allow 48573/udp || true"));
    }

    #[test]
    fn rollback_user_scope_nao_mexe_no_firewall() {
        let s = build_rollback_script(true);
        assert!(s.contains("systemctl --user stop oplhost-udpbd"));
        assert!(!s.contains("ufw"));
    }
}
