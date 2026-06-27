//! Varredura das ISOs no diretório-alvo. Lê `CD/` e `DVD/` e devolve as entradas
//! para o `core` calcular o catálogo (`summarize`) e reconstruir o cache
//! (`OplMeta::rebuild_from`) — o caminho que garante o §6 mesmo sem JSON.

use std::path::Path;

use oplhost_core::GameEntry;

/// Subpastas do OPL que contêm ISOs de jogo.
const GAME_DIRS: [&str; 2] = ["CD", "DVD"];

/// Lista as ISOs presentes em `<root>/CD` e `<root>/DVD`. Diretórios ausentes
/// são ignorados (devolve o que houver); arquivos ilegíveis são pulados. Nunca
/// falha: a UI deve conseguir reconstruir o estado a partir do disco sem erro.
pub fn scan_games(root: &Path) -> Vec<GameEntry> {
    let mut entries = Vec::new();
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
            entries.push(GameEntry {
                file_name: item.file_name().to_string_lossy().into_owned(),
                size_bytes: meta.len(),
            });
        }
    }
    entries
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
}
