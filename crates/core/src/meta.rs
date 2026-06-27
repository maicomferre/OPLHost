//! `opl_meta.json` — metadado DA NOSSA aplicação (não do OPL).
//!
//! Cache de conveniência (§6): nomes, categorias, tamanho, contagem. Vive na
//! raiz do diretório-alvo para portabilidade se o usuário mover o disco.
//! **Requisito crítico:** o app funciona mesmo se o JSON for apagado — daí
//! `rebuild_from`, que reconstrói o estado relendo as ISOs de `CD/`/`DVD/`.
//! Nunca é fonte de verdade única.

use serde::{Deserialize, Serialize};

use crate::catalog::{summarize, CatalogSummary, GameEntry, Media};

/// Versão do schema do `opl_meta.json`. Permite migração futura sem quebrar
/// arquivos antigos quando o cache ganhar campos (ano, art, etc. na Fase 2).
pub const META_VERSION: u32 = 1;

/// Mídia serializável para o cache. Espelha `catalog::Media`, mas com derives de
/// serde — o `catalog` permanece livre de dependências de serialização.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MediaKind {
    Cd,
    Dvd,
}

impl From<Media> for MediaKind {
    fn from(m: Media) -> Self {
        match m {
            Media::Cd => MediaKind::Cd,
            Media::Dvd => MediaKind::Dvd,
        }
    }
}

/// Entrada de um jogo no cache. Campos ricos (título, ano, art) chegam na Fase
/// 2; aqui basta o suficiente para reexibir o catálogo sem reescanear o disco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameMeta {
    pub file_name: String,
    pub size_bytes: u64,
    pub media: MediaKind,
}

impl From<&GameEntry> for GameMeta {
    fn from(e: &GameEntry) -> Self {
        Self {
            file_name: e.file_name.clone(),
            size_bytes: e.size_bytes,
            media: e.media().into(),
        }
    }
}

/// Conteúdo do `opl_meta.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OplMeta {
    pub version: u32,
    pub games: Vec<GameMeta>,
}

impl Default for OplMeta {
    fn default() -> Self {
        Self {
            version: META_VERSION,
            games: Vec::new(),
        }
    }
}

impl OplMeta {
    /// Reconstrói o cache a partir das ISOs lidas do disco. É o caminho usado
    /// quando o JSON está ausente ou corrompido — garante o §6.
    pub fn rebuild_from(entries: &[GameEntry]) -> Self {
        Self {
            version: META_VERSION,
            games: entries.iter().map(GameMeta::from).collect(),
        }
    }

    /// Resumo agregado equivalente ao do catálogo, sem reabrir o disco.
    pub fn summary(&self) -> CatalogSummary {
        let mut s = CatalogSummary::default();
        for g in &self.games {
            s.total_bytes += g.size_bytes;
            match g.media {
                MediaKind::Cd => s.cd_count += 1,
                MediaKind::Dvd => s.dvd_count += 1,
            }
        }
        s
    }

    /// `true` se o cache bate com a varredura atual do disco. A UI usa isto para
    /// decidir se reconstrói (ex.: usuário copiou ISOs por fora do app).
    pub fn matches(&self, entries: &[GameEntry]) -> bool {
        self.summary() == summarize(entries)
    }
}

/// Port de persistência do cache. Implementado na infraestrutura (`JsonMetaStore`);
/// mockável nos testes do `core`.
pub trait MetaStore {
    /// Lê o cache da raiz do diretório-alvo. `Ok(None)` se ainda não existe.
    fn load(&self) -> Result<Option<OplMeta>, MetaError>;
    /// Grava o cache na raiz do diretório-alvo.
    fn save(&self, meta: &OplMeta) -> Result<(), MetaError>;
}

/// Falha de leitura/escrita do cache. Erro de cache nunca deve derrubar o app —
/// a UI segue funcionando reconstruindo a partir do disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaError {
    /// Falha de I/O ao ler/gravar o arquivo.
    Io(String),
    /// JSON malformado — tratar como ausente e reconstruir.
    Malformed(String),
}

impl std::fmt::Display for MetaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetaError::Io(m) => write!(f, "falha de I/O no opl_meta.json: {m}"),
            MetaError::Malformed(m) => write!(f, "opl_meta.json malformado: {m}"),
        }
    }
}

impl std::error::Error for MetaError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<GameEntry> {
        vec![
            GameEntry {
                file_name: "pequeno.iso".into(),
                size_bytes: 100 * 1024 * 1024,
            },
            GameEntry {
                file_name: "grande.iso".into(),
                size_bytes: 4 * 1024 * 1024 * 1024,
            },
        ]
    }

    #[test]
    fn rebuild_from_categoriza_e_preenche_versao() {
        let meta = OplMeta::rebuild_from(&entries());
        assert_eq!(meta.version, META_VERSION);
        assert_eq!(meta.games.len(), 2);
        assert_eq!(meta.games[0].media, MediaKind::Cd);
        assert_eq!(meta.games[1].media, MediaKind::Dvd);
    }

    #[test]
    fn summary_do_cache_bate_com_o_do_catalogo() {
        let es = entries();
        let meta = OplMeta::rebuild_from(&es);
        assert_eq!(meta.summary(), summarize(&es));
        assert!(meta.matches(&es));
    }

    #[test]
    fn matches_falso_quando_o_disco_diverge_do_cache() {
        let meta = OplMeta::rebuild_from(&entries());
        let mut outras = entries();
        outras.pop(); // disco agora tem menos jogos que o cache
        assert!(!meta.matches(&outras));
    }

    #[test]
    fn roundtrip_json_preserva_o_cache() {
        let meta = OplMeta::rebuild_from(&entries());
        let json = serde_json::to_string(&meta).unwrap();
        let voltou: OplMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, voltou);
    }

    #[test]
    fn media_serializa_em_maiusculas() {
        let json = serde_json::to_string(&MediaKind::Dvd).unwrap();
        assert_eq!(json, "\"DVD\"");
    }
}
