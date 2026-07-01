//! Trabalho disparado pelos handlers: o padrão de threading (`spawn_job`) e os
//! jobs concretos (`run_*`) que rodam fora da thread da UI quando bloqueiam
//! (Polkit, rede, diálogo nativo) e devolvem um [`UiUpdate`] para o event loop.

use std::path::{Path, PathBuf};

use oplhost_core::{StorageBackend, create_opl_layout};
use oplhost_infra::{ArtProvider, RealFs, SmbBackend, dialog, iso, scan};
use slint::ComponentHandle;

use crate::AppWindow;
use crate::app_config::{auth_mode, current_status, save_settings, share_config};
use crate::catalog_view::build_catalog;
use crate::dir_hint::dir_hint;
use crate::i18n::{t, t_args};
use crate::ui_update::UiUpdate;

/// Marca `busy`, exibe a mensagem de progresso e roda `job` numa worker thread,
/// devolvendo o `UiUpdate` para o event loop quando terminar. Centraliza o padrão
/// de threading para que os handlers fiquem declarativos.
pub fn spawn_job<F>(ui: &AppWindow, progress: &str, job: F)
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

/// Trabalho de "Ativar" (worker thread): cria a estrutura do OPL e aplica o
/// share SMBv1. `create_opl_layout` é user-space; `apply_config` abre a janela
/// Polkit. Retorna o que a UI deve mostrar.
pub fn run_activate(target: &Path, auth_enabled: bool, password: String) -> UiUpdate {
    if let Err(e) = create_opl_layout(&RealFs, target) {
        return UiUpdate::message(t_args(
            "msg-cannot-create-layout",
            &[
                ("path", target.display().to_string()),
                ("error", e.to_string()),
            ],
        ));
    }

    let cfg = share_config(target, auth_mode(auth_enabled));
    let pw = if auth_enabled { Some(password) } else { None };
    let backend = SmbBackend::new(cfg.clone()).with_auth_password(pw);
    match backend.apply_config(&cfg) {
        Ok(()) => {
            // Config aplicada com sucesso: persiste diretório + toggle (sem senha).
            save_settings(Some(target.to_path_buf()), auth_enabled);
            let (status, active) = current_status();
            let (rows, summary) = build_catalog(target);
            UiUpdate {
                status: Some(status),
                active: Some(active),
                rows: Some(rows),
                summary: Some(summary),
                ..Default::default()
            }
        }
        Err(e) => UiUpdate::message(t_args("msg-cannot-activate", &[("error", e.to_string())])),
    }
}

/// Trabalho de "Desativar" (worker thread): rollback completo via Polkit.
pub fn run_deactivate(target: &Path, auth_enabled: bool) -> UiUpdate {
    let backend = SmbBackend::new(share_config(target, auth_mode(auth_enabled)));
    match backend.rollback() {
        Ok(()) => {
            let (status, active) = current_status();
            UiUpdate {
                status: Some(status),
                active: Some(active),
                message: t("msg-reverted"),
                ..Default::default()
            }
        }
        Err(e) => UiUpdate::message(t_args("msg-cannot-revert", &[("error", e.to_string())])),
    }
}

pub fn run_choose_dir(start: Option<PathBuf>, auth_enabled: bool) -> UiUpdate {
    match dialog::pick_folder(start) {
        Some(path) => {
            // Lembra o novo diretório escolhido para a próxima sessão (sem senha).
            save_settings(Some(path.clone()), auth_enabled);
            let (rows, summary) = build_catalog(&path);
            UiUpdate {
                dir: Some(path.display().to_string()),
                rows: Some(rows),
                summary: Some(summary),
                hint: Some(dir_hint(&path)),
                ..Default::default()
            }
        }
        None => UiUpdate::default(),
    }
}

/// "Baixar capas" (worker thread): para cada ISO do alvo, lê o Game ID e baixa a
/// capa (`COV`) das fontes externas para `ART/`, sem rebaixar o que já existe.
/// Nunca derruba o app (§8) — relata o que conseguiu.
pub fn run_download_art(target: &Path) -> UiUpdate {
    let art_dir = target.join("ART");
    if let Err(e) = std::fs::create_dir_all(&art_dir) {
        return UiUpdate::message(t_args(
            "msg-cannot-create-dir",
            &[
                ("path", art_dir.display().to_string()),
                ("error", e.to_string()),
            ],
        ));
    }

    let provider = ArtProvider::new();
    let (mut downloaded, mut skipped, mut not_found, mut no_id, mut errors) = (0, 0, 0, 0, 0);
    for sg in scan::scan_games_with_paths(target) {
        let id = match iso::read_game_id(&sg.path).ok().flatten() {
            Some(id) => id,
            None => {
                no_id += 1;
                continue;
            }
        };
        match provider.fetch_for_game(&id, &art_dir, false) {
            Ok(out) => {
                downloaded += out.downloaded.len();
                skipped += out.skipped.len();
                not_found += out.not_found.len();
            }
            Err(_) => errors += 1,
        }
    }

    UiUpdate::message(t_args(
        "msg-covers-result",
        &[
            ("downloaded", downloaded.to_string()),
            ("skipped", skipped.to_string()),
            ("notfound", not_found.to_string()),
            ("noid", no_id.to_string()),
            ("errors", errors.to_string()),
        ],
    ))
}
