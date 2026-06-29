//! `FsGameInfoStore` — implementação de `GameInfoStore` sobre os arquivos
//! `CFG/<GameID>.cfg` na raiz do diretório-alvo (onde o OPL lê o info do jogo).
//!
//! **Read-modify-write (regra do `game_info`):** ao salvar, lê o `.cfg` atual,
//! aplica só os 5 campos de info via [`GameCfg`] e regrava — preservando as
//! configs de compatibilidade (`$Compatibility`, `$VMC`…) que moram no mesmo
//! arquivo. `.cfg` ausente vira info vazio (a UI mostra os campos em branco).

use std::path::{Path, PathBuf};

use oplhost_core::{GameCfg, GameId, GameInfo, GameInfoError, GameInfoStore, cfg_file_name};

/// Nome da subpasta do OPL que guarda os `.cfg` por jogo.
pub const CFG_DIR_NAME: &str = "CFG";

/// Persiste o info do jogo em `<target>/CFG/<GameID>.cfg`.
#[derive(Debug, Clone)]
pub struct FsGameInfoStore {
    cfg_dir: PathBuf,
}

impl FsGameInfoStore {
    /// Cria um store apontando para `<target_dir>/CFG`.
    pub fn new(target_dir: &Path) -> Self {
        Self {
            cfg_dir: target_dir.join(CFG_DIR_NAME),
        }
    }

    /// Caminho do `.cfg` de um jogo (não garante existência).
    pub fn cfg_path(&self, game_id: &GameId) -> PathBuf {
        self.cfg_dir.join(cfg_file_name(game_id))
    }

    /// Lê o `.cfg` cru do jogo, ou `""` se ausente. Erro de I/O real (≠
    /// "não existe") sobe como [`GameInfoError::Io`].
    fn read_cfg_raw(&self, game_id: &GameId) -> Result<String, GameInfoError> {
        match std::fs::read_to_string(self.cfg_path(game_id)) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(GameInfoError::Io(e.to_string())),
        }
    }
}

impl GameInfoStore for FsGameInfoStore {
    fn load(&self, game_id: &GameId) -> Result<GameInfo, GameInfoError> {
        Ok(GameCfg::parse(&self.read_cfg_raw(game_id)?).info())
    }

    fn save(&self, game_id: &GameId, info: &GameInfo) -> Result<(), GameInfoError> {
        // Valida antes de tocar o disco: campo > 255 ou com newline quebraria o
        // parse do OPL.
        let errs = info.validate();
        if !errs.is_empty() {
            return Err(GameInfoError::Invalid(errs));
        }

        // Read-modify-write: parte do `.cfg` atual (ou vazio) e mexe só no info.
        let mut cfg = GameCfg::parse(&self.read_cfg_raw(game_id)?);
        cfg.apply_info(info);

        std::fs::create_dir_all(&self.cfg_dir).map_err(|e| GameInfoError::Io(e.to_string()))?;
        std::fs::write(self.cfg_path(game_id), cfg.to_string())
            .map_err(|e| GameInfoError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diretório temporário único por chamada (testes rodam em paralelo).
    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!("oplhost-gameinfo-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn an_id() -> GameId {
        GameId::parse("SLUS_200.02").unwrap()
    }

    #[test]
    fn load_de_cfg_inexistente_e_vazio() {
        let dir = temp_dir();
        let store = FsGameInfoStore::new(&dir);
        assert_eq!(store.load(&an_id()).unwrap(), GameInfo::default());
    }

    #[test]
    fn save_cria_cfg_dir_e_grava_os_campos() {
        let dir = temp_dir();
        let store = FsGameInfoStore::new(&dir);
        let info = GameInfo {
            title: Some("God of War".into()),
            genre: Some("Action".into()),
            ..Default::default()
        };
        store.save(&an_id(), &info).unwrap();

        // Reabre e confere round-trip.
        assert_eq!(store.load(&an_id()).unwrap(), info);
        // Arquivo está em CFG/SLUS_200.02.cfg.
        assert!(dir.join("CFG/SLUS_200.02.cfg").is_file());
    }

    /// TRAVA de integração: salvar info não pode apagar compatibilidade existente.
    #[test]
    fn save_preserva_compatibilidade_existente_no_disco() {
        let dir = temp_dir();
        let store = FsGameInfoStore::new(&dir);
        std::fs::create_dir_all(dir.join("CFG")).unwrap();
        std::fs::write(
            dir.join("CFG/SLUS_200.02.cfg"),
            "$Compatibility=4\n$VMC_0=Save\nGenre=Antigo\n",
        )
        .unwrap();

        store
            .save(
                &an_id(),
                &GameInfo {
                    genre: Some("RPG".into()),
                    developer: Some("Capcom".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let raw = std::fs::read_to_string(dir.join("CFG/SLUS_200.02.cfg")).unwrap();
        assert!(raw.contains("$Compatibility=4"));
        assert!(raw.contains("$VMC_0=Save"));
        assert!(raw.contains("Genre=RPG"));
        assert!(raw.contains("Developer=Capcom"));
        assert!(!raw.contains("Genre=Antigo"));
    }

    #[test]
    fn save_de_campo_invalido_nao_grava_e_retorna_invalid() {
        let dir = temp_dir();
        let store = FsGameInfoStore::new(&dir);
        let info = GameInfo {
            description: Some("a".repeat(300)),
            ..Default::default()
        };
        match store.save(&an_id(), &info) {
            Err(GameInfoError::Invalid(errs)) => assert_eq!(errs.len(), 1),
            other => panic!("esperava Invalid, veio {other:?}"),
        }
        // Nada foi escrito.
        assert!(!dir.join("CFG/SLUS_200.02.cfg").exists());
    }
}
