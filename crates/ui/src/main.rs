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
    create_opl_layout, is_opl_subdir_name, summarize, GameMeta, MediaKind, MetaStore, OplMeta,
    ServerStatus, ShareAuth, ShareConfig, StorageBackend,
};
use oplhost_infra::{dialog, iso, net, scan, ArtProvider, JsonMetaStore, RealFs, SmbBackend};
use slint::{ModelRc, VecModel};

slint::include_modules!();

const SHARE_NAME: &str = "PS2SMB";
const SMB_PORT: u16 = 445;

/// Dados de uma linha do catálogo em Rust puro (Send), convertidos para o
/// `GameRow` do Slint só no event loop. Mantém a worker thread livre de tipos
/// de UI não-`Send`.
struct RowData {
    title: String,
    game_id: String,
    media: String,
    size: String,
}

/// Atualizações de tela produzidas por uma operação na worker thread e aplicadas
/// de volta no event loop. `None` mantém o valor atual da propriedade.
#[derive(Default)]
struct UiUpdate {
    status: Option<String>,
    /// Linha-resumo do catálogo (contagem/tamanho).
    summary: Option<String>,
    /// Linhas do catálogo rico (título/ID/mídia/tamanho).
    rows: Option<Vec<RowData>>,
    /// Novo caminho do diretório-alvo (preenchido pelo seletor de pasta).
    dir: Option<String>,
    /// Dica contextual sobre o diretório-alvo (texto, é_alerta).
    hint: Option<(String, bool)>,
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
        if let Some(summary) = self.summary {
            ui.set_catalog_summary(summary.into());
        }
        if let Some(rows) = self.rows {
            let model: Vec<GameRow> = rows
                .into_iter()
                .map(|r| GameRow {
                    title: r.title.into(),
                    game_id: r.game_id.into(),
                    media: r.media.into(),
                    size: r.size.into(),
                })
                .collect();
            ui.set_catalog_rows(ModelRc::new(VecModel::from(model)));
        }
        if let Some(d) = self.dir {
            ui.set_dir_path(d.into());
        }
        if let Some((text, warning)) = self.hint {
            ui.set_dir_hint(text.into());
            ui.set_dir_hint_warning(warning);
        }
        ui.set_message_text(self.message.into());
        ui.set_busy(false);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    ui.set_ip_text(net::local_ip().unwrap_or_else(|| "indisponível (offline?)".into()).into());
    ui.set_status_text(probe_status_text().into());
    // Usuário do share autenticado = dono da pasta (conta já existente no sistema).
    ui.set_auth_username(current_user().into());
    apply_dir_hint(&ui);

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

    let weak = ui.as_weak();
    ui.on_choose_dir_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handle_choose_dir(&ui);
        }
    });

    let weak = ui.as_weak();
    ui.on_download_art_clicked(move || {
        if let Some(ui) = weak.upgrade() {
            handle_download_art(&ui);
        }
    });

    // Recalcula a dica de pasta enquanto o usuário digita o caminho à mão.
    let weak = ui.as_weak();
    ui.on_dir_path_edited(move || {
        if let Some(ui) = weak.upgrade() {
            apply_dir_hint(&ui);
        }
    });

    ui.run()
}

/// Recalcula e aplica a dica contextual a partir do caminho atual no campo.
/// Leituras de `stat` são instantâneas — pode rodar na thread da UI.
fn apply_dir_hint(ui: &AppWindow) {
    let path = PathBuf::from(ui.get_dir_path().to_string());
    let (text, warning) = dir_hint(&path);
    ui.set_dir_hint(text.into());
    ui.set_dir_hint_warning(warning);
}

/// Dica contextual sobre o diretório-alvo escolhido. Retorna `(texto, é_alerta)`.
///
/// Cobre três casos do feedback de uso real (§ teste Fase 2):
/// 1. caminho vazio → instrução padrão;
/// 2. o usuário apontou uma **subpasta** do OPL (CD/DVD/ART…) em vez da raiz →
///    **alerta**, sugerindo a pasta-pai;
/// 3. a pasta já tem estrutura (CD/ ou DVD/) → nada será recriado; senão, a
///    estrutura será criada ali (só como fallback, não sempre).
fn dir_hint(path: &Path) -> (String, bool) {
    if path.as_os_str().is_empty() {
        return (
            "A estrutura de pastas do OPL (CD, DVD, ART…) é criada aqui se ainda não existir."
                .to_string(),
            false,
        );
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str())
        && is_opl_subdir_name(name)
    {
        let parent = path
            .parent()
            .map(|p| p.display().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "a pasta acima".to_string());
        return (
            format!(
                "Você selecionou a subpasta \"{name}\" da estrutura do OPL. \
                 A raiz do OPL provavelmente é \"{parent}\" — selecione-a."
            ),
            true,
        );
    }

    if path.join("CD").is_dir() || path.join("DVD").is_dir() {
        (
            "Estrutura do OPL detectada nesta pasta — nada será recriado.".to_string(),
            false,
        )
    } else {
        (
            "A estrutura de pastas do OPL (CD, DVD, ART…) será criada aqui, pois ainda não existe."
                .to_string(),
            false,
        )
    }
}

/// Usuário dono da pasta (vira `force user` no share). O app roda em user-space.
fn current_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "nobody".to_string())
}

fn share_config(target: &Path, auth: ShareAuth) -> ShareConfig {
    ShareConfig {
        target_dir: target.to_path_buf(),
        share_name: SHARE_NAME.to_string(),
        port: SMB_PORT,
        owner_user: current_user(),
        auth,
    }
}

/// Modo de acesso a partir do estado da UI: autenticado (usuário = dono da
/// pasta) quando o toggle está ligado, senão guest (padrão).
fn auth_mode(enabled: bool) -> ShareAuth {
    if enabled {
        ShareAuth::User {
            username: current_user(),
        }
    } else {
        ShareAuth::Guest
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

    let auth_enabled = ui.get_auth_enabled();
    let password = ui.get_auth_password().to_string();
    if auth_enabled && password.trim().is_empty() {
        ui.set_message_text(
            "Defina uma senha para o acesso autenticado (ou desmarque a opção).".into(),
        );
        return;
    }

    spawn_job(ui, "Aplicando configuração (informe sua senha no prompt)…", move || {
        run_start(&target, auth_enabled, password)
    });
}

/// "Parar e reverter": rollback completo (remove share + include + firewall) numa
/// única janela Polkit, fora da thread da UI. Volta o sistema ao estado anterior (§0).
fn handle_stop(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string());
    let auth_enabled = ui.get_auth_enabled();

    spawn_job(ui, "Revertendo configuração (informe sua senha no prompt)…", move || {
        run_stop(&target, auth_enabled)
    });
}

/// "Escolher pasta…": abre o seletor nativo (zenity/kdialog) numa worker thread
/// para não travar o event loop, partindo do caminho já digitado. A seleção
/// preenche o campo de diretório; cancelar não altera nada.
fn handle_choose_dir(ui: &AppWindow) {
    let current = ui.get_dir_path().to_string();
    let start = match current.trim() {
        "" => None,
        s => Some(PathBuf::from(s)),
    };
    spawn_job(ui, "Selecionando pasta…", move || run_choose_dir(start));
}

fn run_choose_dir(start: Option<PathBuf>) -> UiUpdate {
    match dialog::pick_folder(start) {
        Some(path) => {
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

/// "Baixar capas": para cada ISO do alvo, lê o Game ID e baixa a capa (`COV`)
/// das fontes externas para `ART/`, sem rebaixar o que já existe. Roda na worker
/// thread (rede bloqueante) e nunca derruba o app (§8) — relata o que conseguiu.
fn handle_download_art(ui: &AppWindow) {
    let target = PathBuf::from(ui.get_dir_path().to_string().trim());
    if target.as_os_str().is_empty() {
        ui.set_message_text("Escolha um diretório-alvo antes de baixar capas.".into());
        return;
    }
    spawn_job(ui, "Baixando capas das fontes externas…", move || {
        run_download_art(&target)
    });
}

fn run_download_art(target: &Path) -> UiUpdate {
    let art_dir = target.join("ART");
    if let Err(e) = std::fs::create_dir_all(&art_dir) {
        return UiUpdate::message(format!("Não foi possível criar {}: {e}", art_dir.display()));
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

    UiUpdate::message(format!(
        "Capas — {downloaded} baixada(s), {skipped} já existia(m), \
         {not_found} sem capa na fonte, {no_id} sem Game ID, {errors} erro(s) de rede."
    ))
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
fn run_start(target: &Path, auth_enabled: bool, password: String) -> UiUpdate {
    if let Err(e) = create_opl_layout(&RealFs, target) {
        return UiUpdate::message(format!(
            "Falha ao criar a estrutura em {}: {e}",
            target.display()
        ));
    }

    let cfg = share_config(target, auth_mode(auth_enabled));
    let pw = if auth_enabled { Some(password) } else { None };
    let backend = SmbBackend::new(cfg.clone()).with_auth_password(pw);
    match backend.apply_config(&cfg) {
        Ok(()) => {
            let (rows, summary) = build_catalog(target);
            UiUpdate {
                status: Some(probe_status_text()),
                rows: Some(rows),
                summary: Some(summary),
                ..Default::default()
            }
        }
        Err(e) => UiUpdate::message(format!("Não foi possível iniciar: {e}")),
    }
}

/// Trabalho de "Parar" (worker thread): rollback completo via Polkit.
fn run_stop(target: &Path, auth_enabled: bool) -> UiUpdate {
    let backend = SmbBackend::new(share_config(target, auth_mode(auth_enabled)));
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
    let backend = SmbBackend::new(share_config(Path::new("/"), ShareAuth::Guest));
    match backend.status() {
        Ok(ServerStatus::Running) => "Rodando",
        Ok(ServerStatus::Stopped) => "Parado",
        Ok(ServerStatus::Error(_)) | Err(_) => "Indeterminado",
    }
    .to_string()
}

/// Lê as ISOs do alvo, extrai o Game ID de cada uma, atualiza o cache
/// `opl_meta.json` e devolve as linhas do catálogo rico + a linha-resumo. Falha
/// de cache é silenciosa: o catálogo vem do disco, não do JSON (§6).
fn build_catalog(target: &Path) -> (Vec<RowData>, String) {
    let scanned = scan::scan_games_with_paths(target);

    let mut metas = Vec::with_capacity(scanned.len());
    for sg in &scanned {
        // Ler o Game ID nunca derruba a listagem: ISO ilegível vira "—".
        let id = iso::read_game_id(&sg.path).ok().flatten();
        metas.push(GameMeta::from_entry(&sg.entry, id));
    }

    let entries: Vec<_> = scanned.into_iter().map(|s| s.entry).collect();
    let summary = summarize(&entries);

    let store = JsonMetaStore::new(target);
    let _ = store.save(&OplMeta::from_games(metas.clone())); // best-effort

    let rows = metas.iter().map(row_from_meta).collect();
    let summary_text = format!(
        "{} jogo(s) — {} CD, {} DVD — {}",
        summary.total_count(),
        summary.cd_count,
        summary.dvd_count,
        format_size(summary.total_bytes)
    );
    (rows, summary_text)
}

/// Converte um `GameMeta` numa linha pronta para exibição.
fn row_from_meta(m: &GameMeta) -> RowData {
    RowData {
        title: if m.title.is_empty() {
            m.file_name.clone()
        } else {
            m.title.clone()
        },
        game_id: m
            .game_id
            .as_ref()
            .map(|g| g.as_str().to_string())
            .unwrap_or_else(|| "—".to_string()),
        media: match m.media {
            MediaKind::Cd => "CD",
            MediaKind::Dvd => "DVD",
        }
        .to_string(),
        size: format_size(m.size_bytes),
    }
}

/// Formata um tamanho em bytes para exibição (MB abaixo de 1 GB, senão GB).
fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", b / MB)
    }
}
