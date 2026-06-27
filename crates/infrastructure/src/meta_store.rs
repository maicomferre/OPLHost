//! `JsonMetaStore` — implementação de `MetaStore` sobre um arquivo JSON na raiz
//! do diretório-alvo (`<target>/opl_meta.json`).
//!
//! Erros são tolerados pela UI: JSON ausente vira `Ok(None)` e JSON malformado
//! vira `MetaError::Malformed` — em ambos os casos o app reconstrói relendo o
//! disco (§6). Nunca deriva o app por causa do cache.

use std::path::{Path, PathBuf};

use oplhost_core::{MetaError, MetaStore, OplMeta};

/// Nome fixo do arquivo de cache na raiz do diretório-alvo.
pub const META_FILE_NAME: &str = "opl_meta.json";

/// Persiste o `opl_meta.json` no diretório-alvo escolhido pelo usuário.
#[derive(Debug, Clone)]
pub struct JsonMetaStore {
    file_path: PathBuf,
}

impl JsonMetaStore {
    /// Cria um store apontando para `<target_dir>/opl_meta.json`.
    pub fn new(target_dir: &Path) -> Self {
        Self {
            file_path: target_dir.join(META_FILE_NAME),
        }
    }

    /// Caminho do arquivo de cache gerenciado.
    pub fn path(&self) -> &Path {
        &self.file_path
    }
}

impl MetaStore for JsonMetaStore {
    fn load(&self) -> Result<Option<OplMeta>, MetaError> {
        let raw = match std::fs::read_to_string(&self.file_path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(MetaError::Io(e.to_string())),
        };
        serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| MetaError::Malformed(e.to_string()))
    }

    fn save(&self, meta: &OplMeta) -> Result<(), MetaError> {
        let json = serde_json::to_string_pretty(meta)
            .map_err(|e| MetaError::Malformed(e.to_string()))?;
        std::fs::write(&self.file_path, json).map_err(|e| MetaError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oplhost_core::{GameEntry, MediaKind};

    /// Diretório temporário único POR CHAMADA — os testes rodam em paralelo e
    /// não podem disputar o mesmo arquivo. Combina PID + contador atômico.
    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!(
            "oplhost-metastore-test-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn load_de_arquivo_inexistente_e_none() {
        let dir = temp_dir();
        let store = JsonMetaStore::new(&dir);
        let _ = std::fs::remove_file(store.path());
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn save_depois_load_preserva_o_cache() {
        let dir = temp_dir();
        let store = JsonMetaStore::new(&dir);
        let meta = OplMeta::rebuild_from(&[GameEntry {
            file_name: "jogo.iso".into(),
            size_bytes: 123,
        }]);

        store.save(&meta).unwrap();
        let voltou = store.load().unwrap().unwrap();

        assert_eq!(voltou, meta);
        assert_eq!(voltou.games[0].media, MediaKind::Cd);
        let _ = std::fs::remove_file(store.path());
    }

    #[test]
    fn json_malformado_vira_erro_malformed() {
        let dir = temp_dir();
        let store = JsonMetaStore::new(&dir);
        std::fs::write(store.path(), "{ isto não é json }").unwrap();

        match store.load() {
            Err(MetaError::Malformed(_)) => {}
            other => panic!("esperava Malformed, veio {other:?}"),
        }
        let _ = std::fs::remove_file(store.path());
    }
}
