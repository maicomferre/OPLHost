//! Construtores PUROS dos scripts do backend SMB.
//!
//! Separados do `SmbBackend` de propósito: gerar o `opl_share.conf` e os corpos
//! de shell de apply/rollback é a parte mais sujeita a erro, então fica em
//! funções puras (sem I/O, sem root) que os testes verificam à exaustão. O
//! `SmbBackend` apenas compõe estes pedaços e os entrega ao `PrivilegeEscalator`.
//!
//! O esqueleto do `opl_share.conf` é o validado na Fase 0 (Samba 4.23.6, NT1
//! guest lê/escreve). Ver `plans/fase-0-spike.md`.

use oplhost_core::{ShareAuth, ShareConfig};

use crate::firewall::{FirewallManager, Protocol};

/// Caminhos dos arquivos de config do Samba que o backend gerencia.
/// `share_conf` é o arquivo ISOLADO do app; `smb_conf` é o global — neste só
/// injetamos/removemos a linha de `include` (§0: nunca editar o conteúdo global).
#[derive(Debug, Clone)]
pub struct SmbPaths {
    pub share_conf: String,
    pub smb_conf: String,
}

impl Default for SmbPaths {
    fn default() -> Self {
        Self {
            share_conf: "/etc/samba/opl_share.conf".to_string(),
            smb_conf: "/etc/samba/smb.conf".to_string(),
        }
    }
}

impl SmbPaths {
    /// Linha de include injetada no `smb.conf` global.
    pub fn include_line(&self) -> String {
        format!("include = {}", self.share_conf)
    }
}

/// Comentário-marcador que delimita a injeção do app no `smb.conf`, para o
/// rollback removê-la sem ambiguidade.
const MARKER: &str = "# oplhost";

/// Gera o conteúdo do `opl_share.conf` que força SMBv1/NT1 (exigência do OPL).
///
/// Esqueleto validado na Fase 0. Avisos de "weak crypto"/"lanman deprecated" do
/// Samba são esperados — é o trade-off do OPL (§0). `smb ports` só aparece em
/// porta não-padrão para não alterar o comportamento default do daemon.
///
/// O bloco de acesso do share ramifica pelo `cfg.auth`: por padrão `guest ok =
/// yes` (acesso livre, como o OPL espera); no modo autenticado, `guest ok = no`
/// com `valid users = <user>` exige usuário/senha. `force user` permanece nos
/// dois casos para que toda escrita pertença ao dono da pasta.
pub fn build_smb_conf(cfg: &ShareConfig) -> String {
    let path = cfg.target_dir.display();
    let smb_ports = if cfg.port == 445 {
        String::new()
    } else {
        format!("   smb ports = {}\n", cfg.port)
    };
    let access = match &cfg.auth {
        ShareAuth::Guest => "\x20  guest ok = yes\n".to_string(),
        ShareAuth::User { username } => {
            format!("\x20  guest ok = no\n\x20  valid users = {username}\n")
        }
    };
    format!(
        "[global]\n\
         {smb_ports}\
         \x20  client min protocol = NT1\n\
         \x20  server min protocol = NT1\n\
         \x20  ntlm auth = yes\n\
         \x20  lanman auth = yes\n\
         \x20  usershare allow guests = yes\n\
         \n\
         [{share}]\n\
         \x20  comment = OPL Share\n\
         \x20  path = {path}\n\
         {access}\
         \x20  read only = no\n\
         \x20  browseable = yes\n\
         \x20  force user = {user}\n",
        share = cfg.share_name,
        user = cfg.owner_user,
    )
}

/// Aspas simples seguras para shell: envolve em `'…'` e escapa aspas simples
/// internas como `'\''`. Usado para interpolar usuário/senha no script root sem
/// risco de injeção.
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

/// Fragmento root que cria/atualiza o usuário Samba do share autenticado.
///
/// Vazio no modo guest (ou sem senha). No modo `User`: guarda contra usuário do
/// sistema inexistente (não criamos contas — abortamos com mensagem clara) e
/// define a senha Samba via `smbpasswd -s -a`, lendo a senha do stdin (nunca em
/// argv, que vazaria no `ps`).
fn build_auth_fragment(cfg: &ShareConfig, password: Option<&str>) -> String {
    match (&cfg.auth, password) {
        (ShareAuth::User { username }, Some(pw)) => {
            let u = sh_squote(username);
            let p = sh_squote(pw);
            format!(
                "if ! id -u {u} >/dev/null 2>&1; then\n\
                 \x20 echo 'usuario do sistema inexistente para o share autenticado' >&2\n\
                 \x20 exit 1\n\
                 fi\n\
                 printf '%s\\n%s\\n' {p} {p} | smbpasswd -s -a {u}\n\
                 \n"
            )
        }
        _ => String::new(),
    }
}

/// Corpo do script root de APPLY: grava o conf isolado, injeta o include de
/// forma idempotente, (opcionalmente) cria o usuário Samba, reinicia o `smbd` e
/// abre a porta no firewall. Tudo numa única janela de privilégio quando
/// entregue ao `PrivilegeEscalator`. `password` só é usado no modo autenticado.
pub fn build_apply_script(paths: &SmbPaths, cfg: &ShareConfig, password: Option<&str>) -> String {
    let conf = build_smb_conf(cfg);
    let include = paths.include_line();
    let firewall = FirewallManager.open_fragment(cfg.port, Protocol::Tcp);
    let auth = build_auth_fragment(cfg, password);
    let share_conf = &paths.share_conf;
    let smb_conf = &paths.smb_conf;

    format!(
        "cat > {share_conf} <<'OPLEOF'\n{conf}OPLEOF\n\
         \n\
         if ! grep -qxF '{include}' {smb_conf}; then\n\
         \x20 printf '\\n{MARKER}\\n{include}\\n' >> {smb_conf}\n\
         fi\n\
         \n\
         {auth}\
         systemctl restart smbd\n\
         \n\
         {firewall}\n"
    )
}

/// Corpo do script root de ROLLBACK: remove o conf isolado, tira a linha de
/// include e o marcador, reinicia o `smbd` e fecha a porta. Rollback completo
/// (§0): o sistema volta ao estado anterior sem vestígios do app.
pub fn build_rollback_script(paths: &SmbPaths, cfg: &ShareConfig) -> String {
    let include = paths.include_line();
    let firewall = FirewallManager.close_fragment(cfg.port, Protocol::Tcp);
    let share_conf = &paths.share_conf;
    let smb_conf = &paths.smb_conf;
    // No modo autenticado, remove a entrada Samba que o apply criou (§0: sem
    // vestígios). `|| true`: a conta pode já ter sido removida; não falhar o rollback.
    let deauth = match &cfg.auth {
        ShareAuth::User { username } => format!("smbpasswd -x {} || true\n", sh_squote(username)),
        ShareAuth::Guest => String::new(),
    };

    format!(
        "rm -f {share_conf}\n\
         sed -i '\\#{include}#d' {smb_conf}\n\
         sed -i '/{MARKER}/d' {smb_conf}\n\
         {deauth}\
         systemctl restart smbd\n\
         \n\
         {firewall}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> ShareConfig {
        ShareConfig {
            target_dir: PathBuf::from("/mnt/ps2games"),
            share_name: "PS2SMB".to_string(),
            port: 445,
            owner_user: "maicom".to_string(),
            auth: ShareAuth::Guest,
        }
    }

    fn auth_cfg() -> ShareConfig {
        ShareConfig {
            auth: ShareAuth::User {
                username: "maicom".to_string(),
            },
            ..cfg()
        }
    }

    #[test]
    fn conf_forca_smbv1_e_guest_no_caminho_certo() {
        let c = build_smb_conf(&cfg());
        assert!(c.contains("client min protocol = NT1"));
        assert!(c.contains("server min protocol = NT1"));
        assert!(c.contains("lanman auth = yes"));
        assert!(c.contains("[PS2SMB]"));
        assert!(c.contains("path = /mnt/ps2games"));
        assert!(c.contains("force user = maicom"));
        assert!(c.contains("guest ok = yes"));
        assert!(!c.contains("valid users"));
    }

    #[test]
    fn conf_autenticado_exige_usuario_e_nega_guest() {
        let c = build_smb_conf(&auth_cfg());
        assert!(c.contains("guest ok = no"));
        assert!(c.contains("valid users = maicom"));
        // força usuário continua, garantindo a posse dos arquivos
        assert!(c.contains("force user = maicom"));
        assert!(!c.contains("guest ok = yes"));
    }

    #[test]
    fn porta_padrao_445_nao_emite_smb_ports() {
        assert!(!build_smb_conf(&cfg()).contains("smb ports"));
    }

    #[test]
    fn porta_alternativa_emite_smb_ports() {
        let mut c = cfg();
        c.port = 1445;
        let conf = build_smb_conf(&c);
        assert!(conf.contains("smb ports = 1445"));
    }

    #[test]
    fn apply_injeta_include_idempotente_e_reinicia_smbd() {
        let s = build_apply_script(&SmbPaths::default(), &cfg(), None);
        // heredoc grava o conf isolado
        assert!(s.contains("cat > /etc/samba/opl_share.conf <<'OPLEOF'"));
        // include idempotente: só anexa se ainda não existe
        assert!(s.contains("grep -qxF 'include = /etc/samba/opl_share.conf' /etc/samba/smb.conf"));
        assert!(s.contains("systemctl restart smbd"));
        // firewall na mesma janela
        assert!(s.contains("ufw allow 445/tcp"));
    }

    #[test]
    fn apply_guest_nao_mexe_em_smbpasswd() {
        let s = build_apply_script(&SmbPaths::default(), &cfg(), Some("ignorada"));
        assert!(!s.contains("smbpasswd"));
    }

    #[test]
    fn apply_autenticado_cria_usuario_samba_na_mesma_janela() {
        let s = build_apply_script(&SmbPaths::default(), &auth_cfg(), Some("s3nha"));
        // guarda contra usuário do sistema inexistente
        assert!(s.contains("id -u 'maicom'"));
        // senha vai pelo stdin (não em argv) e antes do restart
        assert!(s.contains("printf '%s\\n%s\\n' 's3nha' 's3nha' | smbpasswd -s -a 'maicom'"));
        let smbpasswd_at = s.find("smbpasswd -s -a").unwrap();
        let restart_at = s.find("systemctl restart smbd").unwrap();
        assert!(smbpasswd_at < restart_at, "usuário criado antes de reiniciar o smbd");
    }

    #[test]
    fn apply_autenticado_sem_senha_nao_emite_smbpasswd() {
        // segurança: sem senha, não tentamos criar o usuário (evita prompt travado)
        let s = build_apply_script(&SmbPaths::default(), &auth_cfg(), None);
        assert!(!s.contains("smbpasswd"));
    }

    #[test]
    fn senha_com_aspas_simples_e_escapada_para_o_shell() {
        let s = build_apply_script(&SmbPaths::default(), &auth_cfg(), Some("a'b"));
        assert!(s.contains(r"'a'\''b'"));
    }

    #[test]
    fn rollback_remove_conf_include_e_marcador() {
        let s = build_rollback_script(&SmbPaths::default(), &cfg());
        assert!(s.contains("rm -f /etc/samba/opl_share.conf"));
        assert!(s.contains("sed -i '\\#include = /etc/samba/opl_share.conf#d' /etc/samba/smb.conf"));
        assert!(s.contains("# oplhost"));
        assert!(s.contains("systemctl restart smbd"));
        assert!(s.contains("ufw delete allow 445/tcp || true"));
        // modo guest não mexe em conta Samba
        assert!(!s.contains("smbpasswd"));
    }

    #[test]
    fn rollback_autenticado_remove_a_conta_samba() {
        let s = build_rollback_script(&SmbPaths::default(), &auth_cfg());
        assert!(s.contains("smbpasswd -x 'maicom' || true"));
    }
}
