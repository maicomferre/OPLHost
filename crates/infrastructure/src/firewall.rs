//! `FirewallManager` — abre/fecha a porta do backend no firewall.
//!
//! Não executa nada sozinho: produz FRAGMENTOS de shell que entram no mesmo
//! script root do backend, para tudo caber numa única janela Polkit (§5). A
//! detecção `ufw` ativo → senão `iptables` é feita em tempo de execução dentro
//! do fragmento (foi assim que o spike validou), com persistência best-effort.

use std::fmt;

/// Protocolo de transporte da regra. SMB usa TCP; o futuro `UdpbdBackend` (§7.1)
/// usará UDP/48573 — por isso a porta e o protocolo são parametrizados, sem
/// hardcode de "tcp/445" no gerador de regras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "tcp"),
            Protocol::Udp => write!(f, "udp"),
        }
    }
}

/// Gera os fragmentos de firewall. Sem estado: a porta/protocolo vêm do backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct FirewallManager;

impl FirewallManager {
    /// Fragmento que abre `port/proto`: usa `ufw` se ativo, senão `iptables`
    /// (idempotente via `-C` antes de `-I`) e tenta persistir com netfilter.
    pub fn open_fragment(&self, port: u16, proto: Protocol) -> String {
        format!(
            "if ufw status 2>/dev/null | grep -q 'Status: active'; then\n\
             \x20 ufw allow {port}/{proto}\n\
             else\n\
             \x20 iptables -C INPUT -p {proto} --dport {port} -j ACCEPT 2>/dev/null \\\n\
             \x20   || iptables -I INPUT -p {proto} --dport {port} -j ACCEPT\n\
             \x20 (command -v netfilter-persistent >/dev/null && netfilter-persistent save) || true\n\
             fi"
        )
    }

    /// Fragmento que remove a regra aberta por `open_fragment`. Tolerante a
    /// regra ausente (`|| true`): o rollback nunca deve falhar por isso.
    pub fn close_fragment(&self, port: u16, proto: Protocol) -> String {
        format!(
            "if ufw status 2>/dev/null | grep -q 'Status: active'; then\n\
             \x20 ufw delete allow {port}/{proto} || true\n\
             else\n\
             \x20 iptables -D INPUT -p {proto} --dport {port} -j ACCEPT 2>/dev/null || true\n\
             \x20 (command -v netfilter-persistent >/dev/null && netfilter-persistent save) || true\n\
             fi"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_fragment_cobre_ufw_e_iptables_para_a_porta() {
        let frag = FirewallManager.open_fragment(445, Protocol::Tcp);
        assert!(frag.contains("ufw allow 445/tcp"));
        assert!(frag.contains("iptables -I INPUT -p tcp --dport 445 -j ACCEPT"));
        // idempotência do iptables: checa antes de inserir.
        assert!(frag.contains("iptables -C INPUT -p tcp --dport 445 -j ACCEPT"));
    }

    #[test]
    fn close_fragment_e_tolerante_a_regra_ausente() {
        let frag = FirewallManager.close_fragment(445, Protocol::Tcp);
        assert!(frag.contains("ufw delete allow 445/tcp || true"));
        assert!(frag.contains("iptables -D INPUT -p tcp --dport 445 -j ACCEPT 2>/dev/null || true"));
    }

    #[test]
    fn protocolo_udp_aparece_no_fragmento() {
        let frag = FirewallManager.open_fragment(48573, Protocol::Udp);
        assert!(frag.contains("48573/udp"));
        assert!(frag.contains("-p udp --dport 48573"));
    }
}
