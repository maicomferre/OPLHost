//! Binário principal do oplhost — raiz de composição.
//!
//! Aqui (e só aqui) a UI Slint é ligada ao `core`/`infra`. Os componentes de UI
//! não falam com a `infrastructure` diretamente: o binário injeta os adapters e
//! traduz cliques em chamadas de domínio, devolvendo só texto para a tela. Isso
//! mantém a camada de apresentação trocável (Slint → egui) sem tocar o `core`.

use std::path::{Path, PathBuf};

use oplhost_core::{
    create_opl_layout, summarize, MetaStore, OplMeta, ServerStatus, ShareConfig, StorageBackend,
};
use oplhost_infra::{net, scan, JsonMetaStore, RealFs, SmbBackend};

slint::include_modules!();

const SHARE_NAME: &str = "PS2SMB";
const SMB_PORT: u16 = 445;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    ui.set_ip_text(net::local_ip().unwrap_or_else(|| "indisponível (offline?)".into()).into());
    refresh_status(&ui);

    // Cada clique constrói um backend a partir do diretório atual e atualiza a
    // tela com o resultado. `Weak` evita ciclo de referência com a janela.
    let weak = ui.as_weak();
    ui.on_start_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handle_start(&ui);
        }
    });

    let weak = ui.as_weak();
    ui.on_stop_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handle_stop(&ui);
        }
    });

    ui.run()
}

/// Usuário dono da pasta (vira `force user` no share). O app roda em user-space.
fn current_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "nobody".to_string())
}

fn share_config(target: &Path) -> ShareConfig {
    ShareConfig {
        target_dir: target.to_path_buf(),
        share_name: SHARE_NAME.to_string(),
        port: SMB_PORT,
        owner_user: current_user(),
    }
}

/// "Iniciar": cria a estrutura do OPL, aplica o share SMBv1 (uma janela Polkit)
/// e atualiza catálogo/IP. Erros viram mensagem na tela — nunca panic (§8).
fn handle_start(ui: &AppWindow) {
    let dir = ui.get_dir_path().to_string();
    let target = PathBuf::from(dir.trim());
    if target.as_os_str().is_empty() {
        ui.set_message_text("Escolha um diretório-alvo antes de iniciar.".into());
        return;
    }

    // Estrutura de pastas do OPL: operação de user-space, fora da janela root.
    if let Err(e) = create_opl_layout(&RealFs, &target) {
        ui.set_message_text(format!("Falha ao criar a estrutura em {}: {e}", target.display()).into());
        return;
    }

    let cfg = share_config(&target);
    let backend = SmbBackend::new(cfg.clone());

    ui.set_message_text("Aplicando configuração (informe sua senha no prompt)…".into());
    match backend.apply_config(&cfg) {
        Ok(()) => {
            ui.set_message_text("".into());
            refresh_status(ui);
            refresh_catalog(ui, &target);
        }
        Err(e) => ui.set_message_text(format!("Não foi possível iniciar: {e}").into()),
    }
}

/// "Parar e reverter": rollback completo (remove share + include + firewall) numa
/// única janela Polkit. Volta o sistema ao estado anterior (§0).
fn handle_stop(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string());
    let cfg = share_config(&target);
    let backend = SmbBackend::new(cfg);

    ui.set_message_text("Revertendo configuração (informe sua senha no prompt)…".into());
    match backend.rollback() {
        Ok(()) => {
            ui.set_message_text("Configuração revertida. Nada do app permanece no sistema.".into());
            refresh_status(ui);
        }
        Err(e) => ui.set_message_text(format!("Falha ao reverter: {e}").into()),
    }
}

/// Reflete o estado real do `smbd` na UI.
fn refresh_status(ui: &AppWindow) {
    let backend = SmbBackend::new(share_config(Path::new("/")));
    let text = match backend.status() {
        Ok(ServerStatus::Running) => "Rodando",
        Ok(ServerStatus::Stopped) => "Parado",
        Ok(ServerStatus::Error(_)) | Err(_) => "Indeterminado",
    };
    ui.set_status_text(text.into());
}

/// Lê as ISOs do alvo, atualiza o cache `opl_meta.json` e mostra a contagem.
/// Falha de cache é silenciosa: o catálogo vem do disco, não do JSON (§6).
fn refresh_catalog(ui: &AppWindow, target: &Path) {
    let games = scan::scan_games(target);
    let summary = summarize(&games);

    let store = JsonMetaStore::new(target);
    let meta = OplMeta::rebuild_from(&games);
    let _ = store.save(&meta); // best-effort; não bloqueia o fluxo

    ui.set_catalog_text(
        format!(
            "Catálogo: {} jogo(s) — {} em CD, {} em DVD",
            summary.total_count(),
            summary.cd_count,
            summary.dvd_count
        )
        .into(),
    );
}
