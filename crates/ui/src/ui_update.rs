//! Modelo de atualização de tela: o que uma operação (na thread da UI ou numa
//! worker thread) produz, aplicado de volta no event loop com segurança.

use slint::{ModelRc, VecModel};

use crate::{AppWindow, GameRow};

/// Dados de uma linha do catálogo em Rust puro (Send), convertidos para o
/// `GameRow` do Slint só no event loop. Mantém a worker thread livre de tipos
/// de UI não-`Send`.
pub struct RowData {
    pub title: String,
    pub game_id: String,
    pub media: String,
    pub size: String,
    /// Nome do arquivo cru da ISO — exibido no editor de metadados (o `title`
    /// pode ser derivado/sobrescrito).
    pub file_name: String,
}

/// Atualizações de tela produzidas por uma operação na worker thread e aplicadas
/// de volta no event loop. `None` mantém o valor atual da propriedade.
#[derive(Default)]
pub struct UiUpdate {
    pub status: Option<String>,
    /// Novo estado do servidor (config aplicada?) para o botão de toggle refletir.
    pub active: Option<bool>,
    /// Linha-resumo do catálogo (contagem/tamanho).
    pub summary: Option<String>,
    /// Linhas do catálogo rico (título/ID/mídia/tamanho).
    pub rows: Option<Vec<RowData>>,
    /// Novo caminho do diretório-alvo (preenchido pelo seletor de pasta).
    pub dir: Option<String>,
    /// Dica contextual sobre o diretório-alvo (texto, é_alerta).
    pub hint: Option<(String, bool)>,
    pub message: String,
}

impl UiUpdate {
    /// Só uma mensagem (ex.: erro); não mexe em status/catálogo.
    pub fn message(msg: String) -> Self {
        Self {
            message: msg,
            ..Default::default()
        }
    }

    /// Aplica o resultado na UI e libera os botões (`busy = false`).
    pub fn apply_to(self, ui: &AppWindow) {
        if let Some(s) = self.status {
            ui.set_status_text(s.into());
        }
        if let Some(a) = self.active {
            ui.set_server_active(a);
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
                    file_name: r.file_name.into(),
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
