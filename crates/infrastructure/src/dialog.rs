//! Seletor nativo de pasta. Em vez de uma crate de diálogo (que arrastaria
//! build-deps de GTK ou um runtime async via portal), disparamos a ferramenta de
//! diálogo do desktop: `zenity` (GTK/GNOME, padrão nos alvos do projeto) e, na
//! ausência dela, `kdialog` (KDE). É um processo separado, então não bloqueia o
//! event loop quando chamado da worker thread. `zenity` entra no `Depends` do
//! `.deb`. A varredura de tools mantém o adapter coeso com `net`/`scan`.

use std::path::PathBuf;
use std::process::Command;

/// Abre um diálogo nativo para o usuário escolher um diretório. `start` é a pasta
/// inicial (o caminho já digitado, se houver). Retorna `None` se o usuário
/// cancelar OU se nenhuma ferramenta de diálogo estiver instalada — em ambos os
/// casos a UI simplesmente mantém o campo atual.
pub fn pick_folder(start: Option<PathBuf>) -> Option<PathBuf> {
    // `Ok` = a ferramenta rodou (seleção ou cancelamento); `Err` = não instalada,
    // então cai para a próxima.
    if let Ok(selection) = run_zenity(start.as_ref()) {
        return selection;
    }
    run_kdialog(start.as_ref()).unwrap_or(None)
}

/// A ferramenta de diálogo não está instalada (distinta de "usuário cancelou").
struct ToolUnavailable;

fn run_zenity(start: Option<&PathBuf>) -> Result<Option<PathBuf>, ToolUnavailable> {
    let mut cmd = Command::new("zenity");
    cmd.args([
        "--file-selection",
        "--directory",
        "--title=Escolha o diretório-alvo do OPL",
    ]);
    if let Some(p) = start {
        // Barra final faz o zenity abrir DENTRO da pasta, não selecioná-la.
        cmd.arg(format!("--filename={}/", p.display()));
    }
    run_tool(cmd)
}

fn run_kdialog(start: Option<&PathBuf>) -> Result<Option<PathBuf>, ToolUnavailable> {
    let mut cmd = Command::new("kdialog");
    let dir = start
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".to_string());
    cmd.arg("--getexistingdirectory").arg(dir);
    run_tool(cmd)
}

/// Executa a ferramenta e interpreta a saída. `Err(ToolUnavailable)` só quando o
/// binário não existe; qualquer outra falha vira `Ok(None)` (sem seleção).
fn run_tool(mut cmd: Command) -> Result<Option<PathBuf>, ToolUnavailable> {
    match cmd.output() {
        Ok(out) => Ok(parse_output(&out.stdout, out.status.success())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ToolUnavailable),
        Err(_) => Ok(None),
    }
}

/// Converte a saída do diálogo num caminho. Tanto `zenity` quanto `kdialog`
/// imprimem o caminho escolhido com um `\n` final e saem com 0; no cancelamento
/// saem != 0 (ou sem texto). Função pura para ser testável sem abrir GUI.
fn parse_output(stdout: &[u8], success: bool) -> Option<PathBuf> {
    if !success {
        return None;
    }
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim_end_matches(['\n', '\r']).trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saida_com_caminho_e_sucesso_vira_pathbuf() {
        let got = parse_output(b"/mnt/ps2games\n", true);
        assert_eq!(got, Some(PathBuf::from("/mnt/ps2games")));
    }

    #[test]
    fn cancelamento_status_nao_zero_vira_none() {
        assert_eq!(parse_output(b"/mnt/ps2games\n", false), None);
    }

    #[test]
    fn saida_vazia_vira_none_mesmo_com_sucesso() {
        assert_eq!(parse_output(b"\n", true), None);
        assert_eq!(parse_output(b"   ", true), None);
    }
}
