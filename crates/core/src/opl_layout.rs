//! Estruturação de pastas do OPL. Lógica pura: o OPL descobre os jogos pela
//! estrutura de diretórios diretamente, então o app injeta essas pastas na raiz
//! do diretório-alvo escolhido pelo usuário.

use std::path::{Path, PathBuf};

use crate::ports::Fs;

/// Diretórios que o OPL reconhece na raiz do dispositivo/pasta-alvo.
/// CD/DVD = ISOs; ART = capas; THM = temas; VMC = memory cards virtuais;
/// CFG/CHT/LNG/APPS/POPS = configs, cheats, idiomas, homebrew, PS1.
pub const OPL_DIRS: [&str; 10] = [
    "CD", "DVD", "ART", "THM", "VMC", "CFG", "CHT", "LNG", "APPS", "POPS",
];

/// Caminhos absolutos das pastas do OPL sob `root` (sem tocar disco).
pub fn opl_dir_paths(root: &Path) -> Vec<PathBuf> {
    OPL_DIRS.iter().map(|d| root.join(d)).collect()
}

/// Cria a estrutura de pastas do OPL em `root` usando o port `Fs`.
/// Idempotente (apoia-se em `create_dir_all`).
pub fn create_opl_layout(fs: &dyn Fs, root: &Path) -> std::io::Result<()> {
    for path in opl_dir_paths(root) {
        fs.create_dir_all(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashSet;

    /// Mock de `Fs` que apenas registra os diretórios "criados".
    #[derive(Default)]
    struct MockFs {
        created: RefCell<HashSet<PathBuf>>,
    }

    impl Fs for MockFs {
        fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
            self.created.borrow_mut().insert(path.to_path_buf());
            Ok(())
        }
        fn exists(&self, path: &Path) -> bool {
            self.created.borrow().contains(path)
        }
    }

    #[test]
    fn opl_dir_paths_gera_dez_pastas_sob_a_raiz() {
        let root = Path::new("/mnt/ps2");
        let paths = opl_dir_paths(root);
        assert_eq!(paths.len(), 10);
        assert!(paths.contains(&root.join("CD")));
        assert!(paths.contains(&root.join("POPS")));
    }

    #[test]
    fn create_opl_layout_cria_todas_as_pastas_esperadas() {
        let fs = MockFs::default();
        let root = Path::new("/mnt/ps2");

        create_opl_layout(&fs, root).unwrap();

        let created = fs.created.borrow();
        assert_eq!(created.len(), OPL_DIRS.len());
        for dir in OPL_DIRS {
            assert!(
                created.contains(&root.join(dir)),
                "pasta {dir} deveria ter sido criada"
            );
        }
    }

    #[test]
    fn create_opl_layout_e_idempotente_no_contrato() {
        let fs = MockFs::default();
        let root = Path::new("/mnt/ps2");

        create_opl_layout(&fs, root).unwrap();
        create_opl_layout(&fs, root).unwrap();

        // Repetir não muda o conjunto de pastas (mesma estrutura final).
        assert_eq!(fs.created.borrow().len(), OPL_DIRS.len());
    }
}
