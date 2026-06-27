//! Varredura das ISOs no diretório-alvo. Lê `CD/` e `DVD/` e devolve as entradas
//! para o `core` calcular o catálogo (`summarize`) e reconstruir o cache
//! (`OplMeta::rebuild_from`) — o caminho que garante o §6 mesmo sem JSON.

use std::path::{Path, PathBuf};

use oplhost_core::GameEntry;

/// Subpastas do OPL que contêm ISOs de jogo.
const GAME_DIRS: [&str; 2] = ["CD", "DVD"];

/// Uma ISO encontrada no disco, com o caminho real (para ler o Game ID do
/// `SYSTEM.CNF`) e a entrada de catálogo para o `core`.
pub struct ScannedGame {
    pub path: PathBuf,
    pub entry: GameEntry,
}

/// Lista as ISOs de `<root>/CD` e `<root>/DVD` com seus caminhos. Diretórios
/// ausentes são ignorados; arquivos ilegíveis são pulados. Nunca falha — a UI
/// reconstrói o estado a partir do disco sem erro.
pub fn scan_games_with_paths(root: &Path) -> Vec<ScannedGame> {
    let mut out = Vec::new();
    for dir in GAME_DIRS {
        let path = root.join(dir);
        let read = match std::fs::read_dir(&path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for item in read.flatten() {
            let meta = match item.metadata() {
                Ok(m) if m.is_file() => m,
                _ => continue,
            };
            out.push(ScannedGame {
                path: item.path(),
                entry: GameEntry {
                    file_name: item.file_name().to_string_lossy().into_owned(),
                    size_bytes: meta.len(),
                },
            });
        }
    }
    out
}

/// Variante só com as entradas de catálogo (sem os caminhos), para o cálculo
/// puro do `core` (`summarize`, `OplMeta::rebuild_from`).
pub fn scan_games(root: &Path) -> Vec<GameEntry> {
    scan_games_with_paths(root)
        .into_iter()
        .map(|s| s.entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_root() -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!("oplhost-scan-test-{}-{n}", std::process::id()));
        d
    }

    #[test]
    fn diretorio_sem_cd_dvd_devolve_vazio() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();
        assert!(scan_games(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn le_isos_de_cd_e_dvd() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("CD")).unwrap();
        std::fs::create_dir_all(root.join("DVD")).unwrap();
        std::fs::write(root.join("CD/a.iso"), b"xx").unwrap();
        std::fs::write(root.join("DVD/b.iso"), b"yyyy").unwrap();

        let mut games = scan_games(&root);
        games.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        assert_eq!(games.len(), 2);
        assert_eq!(games[0].file_name, "a.iso");
        assert_eq!(games[0].size_bytes, 2);
        assert_eq!(games[1].size_bytes, 4);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn with_paths_aponta_o_arquivo_real() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("CD")).unwrap();
        std::fs::write(root.join("CD/jogo.iso"), b"xx").unwrap();

        let scanned = scan_games_with_paths(&root);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].entry.file_name, "jogo.iso");
        assert_eq!(scanned[0].path, root.join("CD/jogo.iso"));
        assert!(scanned[0].path.is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
