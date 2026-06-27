//! Catálogo do OPL: lógica pura de contagem, tamanho e categorização das ISOs.
//!
//! O OPL separa os jogos por mídia: `CD/` para imagens ≤ 700 MB e `DVD/` para o
//! resto. Aqui não se toca disco — recebe-se a lista de entradas já lida pela
//! infraestrutura e calcula-se o resumo do catálogo. Mantém o `core` testável e
//! independente de filesystem (regra de inversão de dependência do §3).

/// Limite de tamanho (em bytes) que separa um jogo de CD de um de DVD no OPL.
/// 700 MB é a convenção do projeto (§4 do CLAUDE.md).
pub const CD_MAX_BYTES: u64 = 700 * 1024 * 1024;

/// Mídia em que uma ISO é categorizada para o OPL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    /// Imagem pequena (≤ 700 MB) — vai em `CD/`.
    Cd,
    /// Imagem grande (> 700 MB) — vai em `DVD/`.
    Dvd,
}

impl Media {
    /// Pasta do OPL correspondente a esta mídia.
    pub fn dir_name(self) -> &'static str {
        match self {
            Media::Cd => "CD",
            Media::Dvd => "DVD",
        }
    }
}

/// Uma ISO presente no diretório-alvo. Genérico de propósito: descreve o jogo
/// sem amarrar a backend (SMB ou UDPBD veem a mesma estrutura de pastas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameEntry {
    /// Nome do arquivo (ex.: "SLUS_200.02.God of War.iso").
    pub file_name: String,
    /// Tamanho do arquivo em bytes.
    pub size_bytes: u64,
}

impl GameEntry {
    /// Categoriza a entrada pela mídia segundo o limite do OPL.
    pub fn media(&self) -> Media {
        if self.size_bytes <= CD_MAX_BYTES {
            Media::Cd
        } else {
            Media::Dvd
        }
    }
}

/// Resumo agregado do catálogo, pronto para a UI exibir (§8: status e listagem).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogSummary {
    /// Quantidade de jogos de CD (≤ 700 MB).
    pub cd_count: usize,
    /// Quantidade de jogos de DVD (> 700 MB).
    pub dvd_count: usize,
    /// Soma do tamanho em disco de todas as ISOs, em bytes.
    pub total_bytes: u64,
}

impl CatalogSummary {
    /// Total de jogos (CD + DVD).
    pub fn total_count(&self) -> usize {
        self.cd_count + self.dvd_count
    }
}

/// Calcula o resumo do catálogo a partir das entradas lidas pela infraestrutura.
/// Loop explícito por clareza (§3: legibilidade acima de combinadores densos).
pub fn summarize(entries: &[GameEntry]) -> CatalogSummary {
    let mut summary = CatalogSummary::default();
    for entry in entries {
        summary.total_bytes += entry.size_bytes;
        match entry.media() {
            Media::Cd => summary.cd_count += 1,
            Media::Dvd => summary.dvd_count += 1,
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso(name: &str, size: u64) -> GameEntry {
        GameEntry {
            file_name: name.to_string(),
            size_bytes: size,
        }
    }

    #[test]
    fn no_limite_de_700mb_ainda_e_cd() {
        let e = iso("borda.iso", CD_MAX_BYTES);
        assert_eq!(e.media(), Media::Cd);
        assert_eq!(e.media().dir_name(), "CD");
    }

    #[test]
    fn um_byte_acima_do_limite_vira_dvd() {
        let e = iso("grande.iso", CD_MAX_BYTES + 1);
        assert_eq!(e.media(), Media::Dvd);
        assert_eq!(e.media().dir_name(), "DVD");
    }

    #[test]
    fn summarize_conta_e_soma_por_midia() {
        let entries = vec![
            iso("a.iso", 100 * 1024 * 1024),        // CD
            iso("b.iso", CD_MAX_BYTES),             // CD (borda)
            iso("c.iso", 4 * 1024 * 1024 * 1024),   // DVD
        ];
        let s = summarize(&entries);
        assert_eq!(s.cd_count, 2);
        assert_eq!(s.dvd_count, 1);
        assert_eq!(s.total_count(), 3);
        assert_eq!(
            s.total_bytes,
            100 * 1024 * 1024 + CD_MAX_BYTES + 4 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn summarize_de_lista_vazia_e_zerado() {
        let s = summarize(&[]);
        assert_eq!(s, CatalogSummary::default());
        assert_eq!(s.total_count(), 0);
        assert_eq!(s.total_bytes, 0);
    }
}
