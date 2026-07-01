//! Handlers dos callbacks da UI: validam o estado na thread do event loop e,
//! quando a operação bloqueia, delegam para um job (`crate::jobs`). I/O local
//! rápido (ler/gravar `.cfg`, carregar capa) roda aqui mesmo. Erros viram
//! mensagem — nunca panic (§8).

use std::path::PathBuf;

use oplhost_core::{GameId, GameInfo, GameInfoStore, derive_title};
use oplhost_infra::FsGameInfoStore;
use slint::Model;

use crate::AppWindow;
use crate::i18n::{t, t_args};
use crate::jobs::{run_activate, run_choose_dir, run_deactivate, run_download_art, spawn_job};

/// Toggle único do servidor: desativa (rollback) quando a config está aplicada,
/// ativa (apply) quando não está. O estado real vem de `server_active`, coerente
/// com o status exibido — um só botão, sem os dois conflitantes de antes.
pub fn handle_toggle_server(ui: &AppWindow) {
    if ui.get_server_active() {
        handle_deactivate(ui);
    } else {
        handle_activate(ui);
    }
}

/// "Ativar servidor": valida a entrada na thread da UI, marca `busy` e dispara o
/// trabalho bloqueante (Polkit) numa worker thread.
fn handle_activate(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string().trim());
    if target.as_os_str().is_empty() {
        ui.set_message_text(t("msg-choose-dir-before-activate").into());
        return;
    }

    let auth_enabled = ui.get_auth_enabled();
    let password = ui.get_auth_password().to_string();
    if auth_enabled && password.trim().is_empty() {
        ui.set_message_text(t("msg-set-password").into());
        return;
    }

    spawn_job(ui, &t("progress-applying"), move || {
        run_activate(&target, auth_enabled, password)
    });
}

/// "Desativar e reverter": rollback completo (remove share + include + firewall)
/// numa única janela Polkit, fora da thread da UI. Volta o sistema ao estado
/// anterior (§0).
fn handle_deactivate(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string());
    let auth_enabled = ui.get_auth_enabled();

    spawn_job(ui, &t("progress-reverting"), move || {
        run_deactivate(&target, auth_enabled)
    });
}

/// "Escolher pasta…": abre o seletor nativo (zenity/kdialog) numa worker thread
/// para não travar o event loop, partindo do caminho já digitado. A seleção
/// preenche o campo de diretório; cancelar não altera nada.
pub fn handle_choose_dir(ui: &AppWindow) {
    let current = ui.get_dir_path().to_string();
    let start = match current.trim() {
        "" => None,
        s => Some(PathBuf::from(s)),
    };
    let auth_enabled = ui.get_auth_enabled();
    spawn_job(ui, &t("progress-selecting-folder"), move || {
        run_choose_dir(start, auth_enabled)
    });
}

/// "Baixar capas": dispara o job de download numa worker thread (rede
/// bloqueante).
pub fn handle_download_art(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string().trim());
    if target.as_os_str().is_empty() {
        ui.set_message_text(t("msg-choose-dir-before-art").into());
        return;
    }
    spawn_job(ui, &t("progress-downloading-art"), move || {
        run_download_art(&target)
    });
}

/// Clique numa linha do catálogo: abre o editor de metadados in-place do jogo.
/// Lê o `CFG/<GameID>.cfg` atual (rápido, I/O local → na thread da UI) e
/// pré-preenche os campos. Jogo sem Game ID abre só-leitura (sem `.cfg` p/ casar).
pub fn handle_game_clicked(ui: &AppWindow, idx: i32) {
    let rows = ui.get_catalog_rows();
    let Some(row) = (idx >= 0).then(|| rows.row_data(idx as usize)).flatten() else {
        return;
    };
    let target = PathBuf::from(ui.get_dir_path().to_string().trim());

    ui.set_editor_index(idx);
    ui.set_editor_file_name(row.file_name.clone());
    ui.set_editor_media(row.media.clone());
    ui.set_editor_note("".into());
    set_editor_fields(ui, &GameInfo::default());
    ui.set_editor_has_cover(false);

    match GameId::parse(row.game_id.as_str()) {
        Some(id) => {
            ui.set_editor_game_id(id.as_str().into());
            ui.set_editor_can_edit(true);
            match FsGameInfoStore::new(&target).load(&id) {
                Ok(info) => set_editor_fields(ui, &info),
                Err(e) => ui.set_editor_note(
                    t_args("msg-cannot-read-meta", &[("error", e.to_string())]).into(),
                ),
            }
            load_cover(ui, &target, &id);
        }
        None => {
            ui.set_editor_game_id("".into());
            ui.set_editor_can_edit(false);
        }
    }

    ui.set_show_game_editor(true);
}

/// Preenche os 5 campos do editor a partir de um `GameInfo` (campo ausente → "").
fn set_editor_fields(ui: &AppWindow, info: &GameInfo) {
    let s = |o: &Option<String>| o.clone().unwrap_or_default().into();
    ui.set_field_title(s(&info.title));
    ui.set_field_genre(s(&info.genre));
    ui.set_field_release(s(&info.release));
    ui.set_field_developer(s(&info.developer));
    ui.set_field_description(s(&info.description));
}

/// Carrega a capa `ART/<id>_COV.{png,jpg}` no editor, se existir. Falha é
/// silenciosa (capa some) — é só enriquecimento visual.
fn load_cover(ui: &AppWindow, target: &std::path::Path, id: &GameId) {
    let art = target.join("ART");
    for ext in ["png", "jpg"] {
        let path = art.join(format!("{}_COV.{ext}", id.as_str()));
        if path.is_file()
            && let Ok(img) = slint::Image::load_from_path(&path)
        {
            ui.set_editor_cover(img);
            ui.set_editor_has_cover(true);
            return;
        }
    }
}

/// Salvar do editor: monta o `GameInfo` (campo vazio → `None` = remover a chave),
/// valida e grava por read-modify-write em `CFG/<GameID>.cfg` (preserva
/// compatibilidade). I/O local rápido → na thread da UI. Erro vira aviso no
/// editor, sem fechar. Sucesso fecha o editor e atualiza o título da linha.
pub fn handle_save_game_info(ui: &AppWindow) {
    let Some(id) = GameId::parse(ui.get_editor_game_id().as_str()) else {
        ui.set_editor_note(t("msg-no-game-id").into());
        return;
    };

    // Todos os 5 campos são texto livre — o OPL exibe Release verbatim (não
    // parseia data), então não há formato a impor; só o limite de 255 caracteres
    // (validado no `save`) vale para qualquer campo.
    let info = GameInfo {
        title: non_empty(ui.get_field_title().as_str()),
        genre: non_empty(ui.get_field_genre().as_str()),
        release: non_empty(ui.get_field_release().as_str()),
        developer: non_empty(ui.get_field_developer().as_str()),
        description: non_empty(ui.get_field_description().as_str()),
    };

    let target = PathBuf::from(ui.get_dir_path().to_string().trim());
    match FsGameInfoStore::new(&target).save(&id, &info) {
        Ok(()) => {
            update_row_title(ui, &info);
            ui.set_show_game_editor(false);
            ui.set_message_text(
                t_args("msg-meta-saved", &[("id", id.as_str().to_string())]).into(),
            );
        }
        Err(e) => {
            ui.set_editor_note(t_args("msg-cannot-save-meta", &[("error", e.to_string())]).into())
        }
    }
}

/// Atualiza o título exibido na linha editada: usa o `Title` sobrescrito, ou
/// volta ao título derivado do arquivo quando o campo é esvaziado. Mantém a lista
/// coerente com o que o OPL passará a mostrar, sem reler o disco.
fn update_row_title(ui: &AppWindow, info: &GameInfo) {
    let idx = ui.get_editor_index();
    let rows = ui.get_catalog_rows();
    let Some(mut row) = (idx >= 0).then(|| rows.row_data(idx as usize)).flatten() else {
        return;
    };
    let title = info.title.clone().unwrap_or_else(|| {
        let derived = derive_title(row.file_name.as_str());
        if derived.is_empty() {
            row.file_name.to_string()
        } else {
            derived
        }
    });
    row.title = title.into();
    rows.set_row_data(idx as usize, row);
}

/// `Some(texto_trim)` se não-vazio após `trim`; senão `None`. Usado para mapear
/// campo de texto vazio em "remover a chave" no `.cfg`.
fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
