//! Varredura das ISOs no diretório-alvo. Lê `CD/` e `DVD/` e devolve as entradas
//! (com o caminho real) para o `core` calcular o catálogo (`summarize`) e montar
//! o cache — o caminho que garante o §6 mesmo sem JSON.

use std::path::{Path, PathBuf};

use oplhost_core::{GameEntry, is_game_image_name};

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
            // Ignora lixo: só imagens de jogo (extensão do OPL, regra do `core`)
            // e nada de arquivos vazios. Evita entradas como "games — 0 MB".
            let file_name = item.file_name().to_string_lossy().into_owned();
            if meta.len() == 0 || !is_game_image_name(&file_name) {
                continue;
            }
            out.push(ScannedGame {
                path: item.path(),
                entry: GameEntry {
                    file_name,
                    size_bytes: meta.len(),
                },
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::unique_path;

    fn temp_root() -> std::path::PathBuf {
        unique_path("scan")
    }

    /// Só as entradas de catálogo (sem os caminhos), para os asserts.
    fn entries(root: &Path) -> Vec<GameEntry> {
        scan_games_with_paths(root)
            .into_iter()
            .map(|s| s.entry)
            .collect()
    }

    #[test]
    fn diretorio_sem_cd_dvd_devolve_vazio() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();
        assert!(entries(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn le_isos_de_cd_e_dvd() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("CD")).unwrap();
        std::fs::create_dir_all(root.join("DVD")).unwrap();
        std::fs::write(root.join("CD/a.iso"), b"xx").unwrap();
        std::fs::write(root.join("DVD/b.iso"), b"yyyy").unwrap();

        let mut games = entries(&root);
        games.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        assert_eq!(games.len(), 2);
        assert_eq!(games[0].file_name, "a.iso");
        assert_eq!(games[0].size_bytes, 2);
        assert_eq!(games[1].size_bytes, 4);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignora_lixo_sem_extensao_de_jogo_e_arquivos_vazios() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("CD")).unwrap();
        // o caso real: arquivo solto "games" (sem extensão) que virava "— 0 MB"
        std::fs::write(root.join("CD/games"), b"").unwrap();
        std::fs::write(root.join("CD/LEIAME.txt"), b"nota").unwrap();
        std::fs::write(root.join("CD/vazio.iso"), b"").unwrap(); // .iso porém 0 byte
        std::fs::write(root.join("CD/valido.iso"), b"data").unwrap();

        let games = entries(&root);
        assert_eq!(games.len(), 1, "só a ISO não-vazia entra no catálogo");
        assert_eq!(games[0].file_name, "valido.iso");
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
