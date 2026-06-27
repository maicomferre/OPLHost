//! Adapter de filesystem real, sobre `std::fs`.

use std::path::Path;

use oplhost_core::Fs;

/// Implementação concreta de `Fs` que escreve no disco de verdade.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

impl Fs for RealFs {
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
