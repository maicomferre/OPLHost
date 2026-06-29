//! Montagem do catálogo rico para a UI: lê as ISOs do alvo, extrai o Game ID,
//! atualiza o cache `opl_meta.json` e produz as linhas + a linha-resumo.

use std::path::Path;

use oplhost_core::{GameMeta, MediaKind, MetaStore, OplMeta, summarize};
use oplhost_infra::{JsonMetaStore, iso, scan};

use crate::i18n::t_args;
use crate::ui_update::RowData;

/// Lê as ISOs do alvo, extrai o Game ID de cada uma, atualiza o cache
/// `opl_meta.json` e devolve as linhas do catálogo rico + a linha-resumo. Falha
/// de cache é silenciosa: o catálogo vem do disco, não do JSON (§6).
pub fn build_catalog(target: &Path) -> (Vec<RowData>, String) {
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
    let summary_text = t_args(
        "catalog-summary",
        &[
            ("total", summary.total_count().to_string()),
            ("cd", summary.cd_count.to_string()),
            ("dvd", summary.dvd_count.to_string()),
            ("size", format_size(summary.total_bytes)),
        ],
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
        file_name: m.file_name.clone(),
    }
}

/// Formata um tamanho em bytes para exibição (MB abaixo de 1 GB, senão GB).
pub fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", b / MB)
    }
}
