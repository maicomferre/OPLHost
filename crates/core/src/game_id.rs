//! Game ID do PS2 — a chave que o OPL usa para identificar um jogo e nomear o
//! art. Vem do `SYSTEM.CNF` da ISO, na linha `BOOT2 = cdrom0:\SLUS_213.86;1`.
//!
//! Lógica PURA: o `core` só extrai/valida o ID a partir do texto; quem abre a
//! ISO e lê o `SYSTEM.CNF` é a `infrastructure` (regra de inversão do §3).
//! Formato canônico do OPL: 4 letras, `_`, 3 dígitos, `.`, 2 dígitos
//! (ex.: `SLUS_213.86`).

use serde::{Deserialize, Serialize};

/// Identificador de um jogo de PS2 já normalizado (maiúsculas, sem `;versão`).
/// Newtype para impedir que uma string qualquer seja tratada como Game ID válido.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(String);

impl GameId {
    /// Valida e normaliza uma string para Game ID. `None` se não casar o formato
    /// `LLLL_NNN.NN`. Normaliza para maiúsculas (o disco às vezes traz minúsculas).
    pub fn parse(raw: &str) -> Option<GameId> {
        let s = raw.trim().to_ascii_uppercase();
        if is_valid_id(&s) {
            Some(GameId(s))
        } else {
            None
        }
    }

    /// Forma canônica (ex.: `SLUS_213.86`).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `true` se `s` casa exatamente o padrão `LLLL_NNN.NN` (já em maiúsculas).
fn is_valid_id(s: &str) -> bool {
    let b = s.as_bytes();
    // SLUS_213.86 → 4 + 1 + 3 + 1 + 2 = 11 bytes.
    b.len() == 11
        && b[0..4].iter().all(u8::is_ascii_uppercase)
        && b[4] == b'_'
        && b[5..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'.'
        && b[9..11].iter().all(u8::is_ascii_digit)
}

/// Extrai o Game ID do conteúdo de um `SYSTEM.CNF`. Procura a linha `BOOT2`,
/// pega o caminho do executável (ex.: `cdrom0:\SLUS_213.86;1`) e isola o ID:
/// remove o prefixo de dispositivo/pasta (até `:`, `\` ou `/`) e o sufixo de
/// versão `;1`. Tolerante a espaços e a `BOOT2=valor` sem espaços.
pub fn parse_boot2_game_id(system_cnf: &str) -> Option<GameId> {
    for line in system_cnf.lines() {
        let mut kv = line.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        if !key.eq_ignore_ascii_case("BOOT2") {
            continue;
        }
        let value = kv.next().unwrap_or("").trim();
        // Fica com o que vem depois do último separador de caminho.
        let basename = value.rsplit(['\\', '/', ':']).next().unwrap_or(value);
        // Remove o sufixo de versão `;1`.
        let candidate = basename.split(';').next().unwrap_or(basename);
        if let Some(id) = GameId::parse(candidate) {
            return Some(id);
        }
    }
    None
}

/// Extensões de imagem de PS2 reconhecidas ao derivar o título do nome do arquivo.
const IMAGE_EXTS: [&str; 4] = ["iso", "zso", "cso", "bin"];

/// Remove a extensão de imagem (se houver) do nome do arquivo.
fn strip_image_ext(name: &str) -> &str {
    if let Some(dot) = name.rfind('.') {
        let ext = &name[dot + 1..];
        if IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
            return &name[..dot];
        }
    }
    name
}

/// Deriva um título legível do nome do arquivo da ISO. Segue a convenção do
/// OPL/PyOPLM `<GameID>.<Título>.iso` (ex.: `SLUS_200.02.God of War.iso` →
/// `God of War`): tira a extensão e, se o nome começar com um Game ID seguido de
/// `.`, remove esse prefixo. Sem o prefixo, usa o nome sem extensão como título.
pub fn derive_title(file_name: &str) -> String {
    let stem = strip_image_ext(file_name);
    if let (Some(head), Some(rest)) = (stem.get(..11), stem.get(11..))
        && let Some(title) = rest.strip_prefix('.')
        && !title.is_empty()
        && GameId::parse(head).is_some()
    {
        return title.trim().to_string();
    }
    stem.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normaliza_para_maiusculas() {
        assert_eq!(
            GameId::parse("slus_213.86").unwrap().as_str(),
            "SLUS_213.86"
        );
    }

    #[test]
    fn rejeita_formatos_invalidos() {
        assert!(GameId::parse("SLUS21386").is_none()); // sem separadores
        assert!(GameId::parse("SLUS_213.8").is_none()); // dígitos a menos
        assert!(GameId::parse("SL1S_213.86").is_none()); // dígito no prefixo
        assert!(GameId::parse("").is_none());
    }

    #[test]
    fn boot2_caminho_completo_com_versao() {
        let cnf = "BOOT2 = cdrom0:\\SLUS_213.86;1\nVER = 1.00\nVMODE = NTSC\n";
        assert_eq!(parse_boot2_game_id(cnf).unwrap().as_str(), "SLUS_213.86");
    }

    #[test]
    fn boot2_sem_espacos_e_barra_unix() {
        let cnf = "BOOT2=cdrom0:/SCUS_972.00;1\n";
        assert_eq!(parse_boot2_game_id(cnf).unwrap().as_str(), "SCUS_972.00");
    }

    #[test]
    fn ignora_outras_chaves_e_boot_sem_2() {
        let cnf = "BOOT = cdrom0:\\SLUS_999.99;1\nVER = 1.00\n";
        assert!(parse_boot2_game_id(cnf).is_none());
    }

    #[test]
    fn system_cnf_sem_boot2_retorna_none() {
        assert!(parse_boot2_game_id("VER = 1.00\nVMODE = NTSC\n").is_none());
    }

    #[test]
    fn derive_title_remove_prefixo_de_game_id_e_extensao() {
        assert_eq!(derive_title("SLUS_200.02.God of War.iso"), "God of War");
        assert_eq!(
            derive_title("scus_973.13.gran turismo 4.ZSO"),
            "gran turismo 4"
        );
    }

    #[test]
    fn derive_title_sem_prefixo_usa_nome_sem_extensao() {
        assert_eq!(
            derive_title("Shadow of the Colossus.iso"),
            "Shadow of the Colossus"
        );
        assert_eq!(
            derive_title("sem_extensao_conhecida.dat"),
            "sem_extensao_conhecida.dat"
        );
    }
}
