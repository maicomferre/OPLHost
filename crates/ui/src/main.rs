//! Binário principal do oplhost — raiz de composição.
//!
//! Aqui (e só aqui) a UI Slint é ligada ao `core`/`infra`. Os componentes de UI
//! não falam com a `infrastructure` diretamente: o binário injeta os adapters e
//! traduz cliques em chamadas de domínio, devolvendo só texto para a tela. Isso
//! mantém a camada de apresentação trocável (Slint → egui) sem tocar o `core`.
//!
//! A lógica vive em módulos especializados: `ui_update` (modelo de atualização
//! de tela), `catalog_view` (montagem do catálogo), `dir_hint` (dica de pasta),
//! `app_config` (glue de config/status), `jobs` (threading + trabalho
//! bloqueante) e `handlers` (corpo dos callbacks). `main` só faz o wiring.

mod app_config;
mod catalog_view;
mod dir_hint;
mod handlers;
mod i18n;
mod jobs;
mod ui_update;

use app_config::{current_status, current_user, load_settings};
use catalog_view::build_catalog;
use dir_hint::apply_dir_hint;
use i18n::t;
use ui_update::UiUpdate;

use oplhost_core::BackendKind;
use oplhost_infra::net;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    // Detecta o idioma do SO e aplica às traduções do Slint (@tr) + fluent (Rust).
    // Logo após new() para o Slint reavaliar os @tr reativamente.
    i18n::init();

    ui.set_ip_text(
        net::local_ip()
            .unwrap_or_else(|| t("ip-unavailable"))
            .into(),
    );
    // Resumo do catálogo começa "vazio" (sobrescrito quando um diretório carrega).
    ui.set_catalog_summary(t("catalog-empty").into());
    let (status, active) = current_status();
    ui.set_status_text(status.into());
    ui.set_server_active(active);
    // Usuário do share autenticado = dono da pasta (conta já existente no sistema).
    ui.set_auth_username(current_user().into());

    // Restaura o estado não-sensível da última sessão (diretório-alvo + toggle de
    // auth) do config.json (XDG). Config ausente/corrompida cai em default — o app
    // funciona normalmente sem ela (§6). A senha NUNCA é restaurada: vive no Samba.
    let restored = load_settings();
    if let Some(dir) = &restored.last_target_dir {
        ui.set_dir_path(dir.display().to_string().into());
    }
    ui.set_auth_enabled(restored.auth_required);
    // Restaura o backend escolhido + o device do UDPBD (o alvo do UDPBD é
    // separado do diretório do SMB — modelos diferentes).
    ui.set_backend_udpbd(matches!(restored.backend_kind, BackendKind::Udpbd));
    if let Some(dev) = &restored.udpbd_device {
        ui.set_udpbd_device(dev.display().to_string().into());
    }
    apply_dir_hint(&ui);

    // Se havia um diretório válido salvo, recarrega o catálogo dele AGORA, antes
    // de `run()` (leitura de disco, sem Polkit). Síncrono de propósito: evita o
    // flash de "catálogo vazio" → "preenchido" que um job de background causaria
    // logo após o show. Para bibliotecas típicas a leitura é instantânea.
    // (O dimensionamento da janela NÃO depende disto: a lista é `vertical-stretch`,
    // então a janela nasce com a `preferred-height` do `.slint`, não com a altura
    // do conteúdo — ver o comentário da janela em app.slint.)
    if let Some(dir) = restored.last_target_dir.filter(|d| d.is_dir()) {
        let (rows, summary) = build_catalog(&dir);
        UiUpdate {
            rows: Some(rows),
            summary: Some(summary),
            ..Default::default()
        }
        .apply_to(&ui);
    }

    // Controle único do servidor: o mesmo botão ativa (apply) ou desativa
    // (rollback) conforme o estado real, evitando os dois botões conflitantes.
    let weak = ui.as_weak();
    ui.on_toggle_server_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_toggle_server(&ui);
        }
    });

    let weak = ui.as_weak();
    ui.on_choose_dir_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_choose_dir(&ui);
        }
    });

    let weak = ui.as_weak();
    ui.on_download_art_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_download_art(&ui);
        }
    });

    // Recalcula a dica de pasta enquanto o usuário digita o caminho à mão.
    let weak = ui.as_weak();
    ui.on_dir_path_edited(move || {
        if let Some(ui) = weak.upgrade() {
            apply_dir_hint(&ui);
        }
    });

    // Troca de backend (SMB↔UDPBD) nos Settings → persiste e reavalia o status.
    let weak = ui.as_weak();
    ui.on_backend_changed(move || {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_backend_changed(&ui);
        }
    });

    // Edição do device/imagem do UDPBD → persiste para a próxima sessão.
    let weak = ui.as_weak();
    ui.on_udpbd_device_edited(move || {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_udpbd_device_edited(&ui);
        }
    });

    // Clique numa linha do catálogo → carrega o info do jogo e abre o editor.
    let weak = ui.as_weak();
    ui.on_game_clicked(move |idx| {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_game_clicked(&ui, idx);
        }
    });

    // Salvar os metadados editados no CFG/<GameID>.cfg do jogo selecionado.
    let weak = ui.as_weak();
    ui.on_save_game_info_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handlers::handle_save_game_info(&ui);
        }
    });

    ui.run()
}
