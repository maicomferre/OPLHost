//! Binário principal do oplhost — raiz de composição.
//!
//! Aqui (e só aqui) a UI Slint é ligada ao `core`/`infra`. Os componentes de UI
//! não falam com a `infrastructure` diretamente: o binário injeta os adapters e
//! traduz cliques em chamadas de domínio, devolvendo só texto para a tela. Isso
//! mantém a camada de apresentação trocável (Slint → egui) sem tocar o `core`.
//!
//! As operações que chamam `pkexec` (apply/rollback) BLOQUEIam até o usuário
//! responder ao prompt do Polkit. Por isso rodam numa worker thread: a thread do
//! event loop nunca trava. O resultado volta para a UI via
//! `Weak::upgrade_in_event_loop`, e a flag `busy` desabilita os botões enquanto
//! a operação corre (evita reentrância/cliques duplos).

use std::path::{Path, PathBuf};

use oplhost_core::{
    create_opl_layout, summarize, MetaStore, OplMeta, ServerStatus, ShareConfig, StorageBackend,
};
use oplhost_infra::{net, scan, JsonMetaStore, RealFs, SmbBackend};

slint::include_modules!();

const SHARE_NAME: &str = "PS2SMB";
const SMB_PORT: u16 = 445;

/// Atualizações de tela produzidas por uma operação na worker thread e aplicadas
/// de volta no event loop. `None` mantém o valor atual da propriedade.
#[derive(Default)]
struct UiUpdate {
    status: Option<String>,
    catalog: Option<String>,
    message: String,
}

impl UiUpdate {
    /// Só uma mensagem (ex.: erro); não mexe em status/catálogo.
    fn message(msg: String) -> Self {
        Self {
            message: msg,
            ..Default::default()
        }
    }

    /// Aplica o resultado na UI e libera os botões (`busy = false`).
    fn apply_to(self, ui: &AppWindow) {
        if let Some(s) = self.status {
            ui.set_status_text(s.into());
        }
        if let Some(c) = self.catalog {
            ui.set_catalog_text(c.into());
        }
        ui.set_message_text(self.message.into());
        ui.set_busy(false);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    ui.set_ip_text(net::local_ip().unwrap_or_else(|| "indisponível (offline?)".into()).into());
    ui.set_status_text(probe_status_text().into());

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

/// "Iniciar": valida a entrada na thread da UI, marca `busy` e dispara o trabalho
/// bloqueante (Polkit) numa worker thread. Erros viram mensagem — nunca panic (§8).
fn handle_start(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string().trim());
    if target.as_os_str().is_empty() {
        ui.set_message_text("Escolha um diretório-alvo antes de iniciar.".into());
        return;
    }

    spawn_job(ui, "Aplicando configuração (informe sua senha no prompt)…", move || {
        run_start(&target)
    });
}

/// "Parar e reverter": rollback completo (remove share + include + firewall) numa
/// única janela Polkit, fora da thread da UI. Volta o sistema ao estado anterior (§0).
fn handle_stop(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string());

    spawn_job(ui, "Revertendo configuração (informe sua senha no prompt)…", move || {
        run_stop(&target)
    });
}

/// Marca `busy`, exibe a mensagem de progresso e roda `job` numa worker thread,
/// devolvendo o `UiUpdate` para o event loop quando terminar. Centraliza o padrão
/// de threading para que `handle_start`/`handle_stop` fiquem declarativos.
fn spawn_job<F>(ui: &AppWindow, progress: &str, job: F)
where
    F: FnOnce() -> UiUpdate + Send + 'static,
{
    ui.set_busy(true);
    ui.set_message_text(progress.into());

    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let update = job();
        // Volta para a thread do event loop para mexer na UI com segurança.
        let _ = weak.upgrade_in_event_loop(move |ui| update.apply_to(&ui));
    });
}

/// Trabalho de "Iniciar" (worker thread): cria a estrutura do OPL e aplica o
/// share SMBv1. `create_opl_layout` é user-space; `apply_config` abre a janela
/// Polkit. Retorna o que a UI deve mostrar.
fn run_start(target: &Path) -> UiUpdate {
    if let Err(e) = create_opl_layout(&RealFs, target) {
        return UiUpdate::message(format!(
            "Falha ao criar a estrutura em {}: {e}",
            target.display()
        ));
    }

    let cfg = share_config(target);
    let backend = SmbBackend::new(cfg.clone());
    match backend.apply_config(&cfg) {
        Ok(()) => UiUpdate {
            status: Some(probe_status_text()),
            catalog: Some(build_catalog_text(target)),
            message: String::new(),
        },
        Err(e) => UiUpdate::message(format!("Não foi possível iniciar: {e}")),
    }
}

/// Trabalho de "Parar" (worker thread): rollback completo via Polkit.
fn run_stop(target: &Path) -> UiUpdate {
    let backend = SmbBackend::new(share_config(target));
    match backend.rollback() {
        Ok(()) => UiUpdate {
            status: Some(probe_status_text()),
            message: "Configuração revertida. Nada do app permanece no sistema.".to_string(),
            ..Default::default()
        },
        Err(e) => UiUpdate::message(format!("Falha ao reverter: {e}")),
    }
}

/// Estado real do `smbd` como texto para a UI. Sem root (`systemctl is-active`).
fn probe_status_text() -> String {
    let backend = SmbBackend::new(share_config(Path::new("/")));
    match backend.status() {
        Ok(ServerStatus::Running) => "Rodando",
        Ok(ServerStatus::Stopped) => "Parado",
        Ok(ServerStatus::Error(_)) | Err(_) => "Indeterminado",
    }
    .to_string()
}

/// Lê as ISOs do alvo, atualiza o cache `opl_meta.json` e devolve a contagem.
/// Falha de cache é silenciosa: o catálogo vem do disco, não do JSON (§6).
fn build_catalog_text(target: &Path) -> String {
    let games = scan::scan_games(target);
    let summary = summarize(&games);

    let store = JsonMetaStore::new(target);
    let meta = OplMeta::rebuild_from(&games);
    let _ = store.save(&meta); // best-effort; não bloqueia o fluxo

    format!(
        "Catálogo: {} jogo(s) — {} em CD, {} em DVD",
        summary.total_count(),
        summary.cd_count,
        summary.dvd_count
    )
}
