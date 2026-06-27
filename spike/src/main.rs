//! Spike da Fase 0 — prova de conceito DESCARTÁVEL.
//!
//! Não tem arquitetura por design. Objetivo único: validar que um servidor
//! Samba configurado para SMBv1 (NT1) aceita conexão guest, condição para o
//! Open PS2 Loader funcionar. Tudo o que mexe em root é agrupado numa ÚNICA
//! janela de privilégio (um `pkexec`), conforme a regra do projeto.
//!
//! Uso:
//!   cargo run -- apply      # configura share SMBv1 + firewall (pede senha 1x)
//!   cargo run -- rollback   # desfaz tudo (pede senha 1x)
//!
//! Após `apply`, validar localmente com:
//!   smbclient //localhost/PS2SMB -N \
//!     --option='client min protocol=NT1' \
//!     --option='client max protocol=NT1' -c 'ls'

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SHARE_CONF: &str = "/etc/samba/opl_share.conf";
const SMB_CONF: &str = "/etc/samba/smb.conf";
const INCLUDE_LINE: &str = "include = /etc/samba/opl_share.conf";
const SHARE_NAME: &str = "PS2SMB";

fn main() {
    let action = env::args().nth(1).unwrap_or_default();
    let result = match action.as_str() {
        "apply" => apply(),
        "rollback" => rollback(),
        _ => {
            eprintln!("uso: oplhost-spike <apply|rollback>");
            std::process::exit(2);
        }
    };

    match result {
        Ok(()) => {
            if action == "apply" {
                println!("\nOK. Valide a conexão SMBv1 local com:");
                println!(
                    "  smbclient //localhost/{SHARE_NAME} -N \\\n    \
                     --option='client min protocol=NT1' \\\n    \
                     --option='client max protocol=NT1' -c 'ls'"
                );
            }
        }
        Err(e) => {
            eprintln!("ERRO: {e}");
            std::process::exit(1);
        }
    }
}

/// Usuário e home do dono do share (o app roda em user-space).
fn current_user() -> Result<(String, String), String> {
    let user = env::var("USER").map_err(|_| "variável USER ausente".to_string())?;
    let home = env::var("HOME").map_err(|_| "variável HOME ausente".to_string())?;
    Ok((user, home))
}

/// Conteúdo do share isolado que força protocolo legado SMBv1/NT1.
///
/// VALIDAR NO AMBIENTE: `lanman auth`/`ntlm auth` estão deprecados no Samba
/// 4.23 e o testparm vai avisar ("Weak crypto is allowed"). Isso é ESPERADO —
/// é exatamente o que o OPL exige. Ajustar aqui até o smbclient NT1 conectar.
fn share_conf(user: &str, share_path: &str) -> String {
    format!(
        "[global]\n\
         \x20  client min protocol = NT1\n\
         \x20  server min protocol = NT1\n\
         \x20  ntlm auth = yes\n\
         \x20  lanman auth = yes\n\
         \x20  usershare allow guests = yes\n\
         \n\
         [{SHARE_NAME}]\n\
         \x20  comment = OPL Share (spike)\n\
         \x20  path = {share_path}\n\
         \x20  guest ok = yes\n\
         \x20  read only = no\n\
         \x20  browseable = yes\n\
         \x20  force user = {user}\n"
    )
}

/// Escreve um script de shell temporário e o executa numa única janela
/// `pkexec`, agrupando todas as operações root.
fn run_root_script(body: &str) -> Result<(), String> {
    let mut path: PathBuf = env::temp_dir();
    path.push(format!("oplhost-spike-{}.sh", std::process::id()));

    let script = format!("#!/bin/bash\nset -euo pipefail\n{body}\n");
    fs::write(&path, script).map_err(|e| format!("falha ao escrever script temp: {e}"))?;

    let status = Command::new("pkexec")
        .arg("/bin/bash")
        .arg(&path)
        .status()
        .map_err(|e| format!("falha ao invocar pkexec: {e}"))?;

    let _ = fs::remove_file(&path);

    if status.success() {
        Ok(())
    } else {
        Err(format!("script root falhou (status {status})"))
    }
}

fn apply() -> Result<(), String> {
    let (user, home) = current_user()?;
    let share_path = format!("{home}/oplhost-spike-share");

    // Pasta-alvo pode ser criada como usuário comum (fica fora da janela root).
    fs::create_dir_all(&share_path)
        .map_err(|e| format!("falha ao criar {share_path}: {e}"))?;

    let conf = share_conf(&user, &share_path);

    // Heredoc com o conteúdo do conf, injeção idempotente do include,
    // restart do smbd e abertura da porta 445 (ufw ou fallback iptables).
    let body = format!(
        "cat > {SHARE_CONF} <<'OPLEOF'\n{conf}OPLEOF\n\
         \n\
         if ! grep -qxF '{INCLUDE_LINE}' {SMB_CONF}; then\n\
         \x20 printf '\\n# oplhost (spike)\\n{INCLUDE_LINE}\\n' >> {SMB_CONF}\n\
         fi\n\
         \n\
         systemctl restart smbd\n\
         \n\
         if ufw status 2>/dev/null | grep -q 'Status: active'; then\n\
         \x20 ufw allow 445/tcp\n\
         else\n\
         \x20 iptables -C INPUT -p tcp --dport 445 -j ACCEPT 2>/dev/null \\\n\
         \x20   || iptables -I INPUT -p tcp --dport 445 -j ACCEPT\n\
         fi\n"
    );

    println!("Aplicando configuração SMBv1 (uma janela de privilégio Polkit)...");
    run_root_script(&body)
}

fn rollback() -> Result<(), String> {
    // Remove o conf isolado, tira a linha de include, reinicia o smbd e
    // remove a regra de firewall. Não apaga a pasta de teste do usuário.
    let body = format!(
        "rm -f {SHARE_CONF}\n\
         sed -i '\\#{INCLUDE_LINE}#d' {SMB_CONF}\n\
         sed -i '/# oplhost (spike)/d' {SMB_CONF}\n\
         systemctl restart smbd\n\
         \n\
         if ufw status 2>/dev/null | grep -q 'Status: active'; then\n\
         \x20 ufw delete allow 445/tcp || true\n\
         else\n\
         \x20 iptables -D INPUT -p tcp --dport 445 -j ACCEPT 2>/dev/null || true\n\
         fi\n"
    );

    println!("Revertendo configuração do spike (uma janela de privilégio Polkit)...");
    run_root_script(&body)
}
