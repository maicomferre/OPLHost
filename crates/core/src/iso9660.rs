//! Parser ISO9660 mínimo e PURO — só o necessário para achar o `SYSTEM.CNF` na
//! raiz de uma ISO de PS2: ler o Primary Volume Descriptor (PVD), pegar o extent
//! do diretório raiz e varrer seus registros.
//!
//! Opera sobre fatias de bytes (`&[u8]`) que a `infrastructure` lê do disco — o
//! `core` não abre arquivo nenhum (regra de inversão do §3). Campos numéricos do
//! ISO9660 são gravados em both-endian; lemos a metade little-endian.

/// Tamanho lógico de setor do ISO9660 (2048 bytes).
pub const SECTOR_SIZE: usize = 2048;

/// LBA fixo do Primary Volume Descriptor.
pub const PVD_LBA: u32 = 16;

/// Um registro de diretório (arquivo ou subdiretório) já decodificado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirRecord {
    /// Nome como gravado no disco (pode incluir `;1` de versão).
    pub name: String,
    /// LBA onde começam os dados (em setores de `SECTOR_SIZE`).
    pub extent_lba: u32,
    /// Tamanho dos dados em bytes.
    pub size: u32,
    /// `true` se o registro é um subdiretório.
    pub is_dir: bool,
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Decodifica um único registro de diretório a partir do seu início em `rec`.
/// `None` se o registro for inválido/curto demais.
pub fn parse_dir_record(rec: &[u8]) -> Option<DirRecord> {
    if rec.len() < 34 {
        return None;
    }
    let len = rec[0] as usize;
    if len < 34 || len > rec.len() {
        return None;
    }
    let extent_lba = le_u32(&rec[2..6]);
    let size = le_u32(&rec[10..14]);
    let is_dir = rec[25] & 0x02 != 0;
    let name_len = rec[32] as usize;
    if 33 + name_len > len {
        return None;
    }
    let name = decode_name(&rec[33..33 + name_len]);
    Some(DirRecord {
        name,
        extent_lba,
        size,
        is_dir,
    })
}

/// Nomes especiais: um único byte 0x00 = "." (próprio diretório) e 0x01 = ".."
/// (pai). Os demais são ASCII direto.
fn decode_name(raw: &[u8]) -> String {
    match raw {
        [0x00] => ".".to_string(),
        [0x01] => "..".to_string(),
        _ => String::from_utf8_lossy(raw).into_owned(),
    }
}

/// Lê o registro do diretório raiz embutido no PVD. Valida a assinatura
/// (`type == 1`, `"CD001"`). O registro-raiz fica no offset 156, com 34 bytes.
pub fn parse_root_record(pvd: &[u8]) -> Option<DirRecord> {
    if pvd.len() < 190 || pvd[0] != 1 || &pvd[1..6] != b"CD001" {
        return None;
    }
    parse_dir_record(&pvd[156..190])
}

/// Varre todos os registros de um extent de diretório. Um byte de comprimento 0
/// indica padding até o próximo setor (registros não cruzam fronteira de setor).
pub fn parse_dir_records(data: &[u8]) -> Vec<DirRecord> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let len = data[pos] as usize;
        if len == 0 {
            // Pula o resto do setor atual.
            let next = (pos / SECTOR_SIZE + 1) * SECTOR_SIZE;
            if next <= pos {
                break;
            }
            pos = next;
            continue;
        }
        if pos + len > data.len() {
            break;
        }
        if let Some(rec) = parse_dir_record(&data[pos..pos + len]) {
            out.push(rec);
        }
        pos += len;
    }
    out
}

/// Compara o nome de um registro com o procurado, ignorando o sufixo de versão
/// `;1` e a caixa (ISO9660 grava nomes em maiúsculas).
pub fn name_matches(record_name: &str, wanted: &str) -> bool {
    let base = record_name.split(';').next().unwrap_or(record_name);
    base.eq_ignore_ascii_case(wanted)
}

/// Localiza um arquivo pelo nome num conjunto de registros já varridos.
pub fn find_file<'a>(records: &'a [DirRecord], wanted: &str) -> Option<&'a DirRecord> {
    records
        .iter()
        .find(|r| !r.is_dir && name_matches(&r.name, wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um registro de diretório sintético com nome e extent dados.
    fn make_record(name: &str, extent: u32, size: u32, is_dir: bool) -> Vec<u8> {
        let name_bytes = name.as_bytes();
        let len = 33 + name_bytes.len();
        // Padroniza para tamanho par (ISO alinha, mas não precisamos aqui).
        let mut rec = vec![0u8; len];
        rec[0] = len as u8;
        rec[2..6].copy_from_slice(&extent.to_le_bytes());
        rec[10..14].copy_from_slice(&size.to_le_bytes());
        rec[25] = if is_dir { 0x02 } else { 0x00 };
        rec[32] = name_bytes.len() as u8;
        rec[33..].copy_from_slice(name_bytes);
        rec
    }

    #[test]
    fn parse_dir_record_le_extent_tamanho_e_nome() {
        let rec = make_record("SYSTEM.CNF;1", 24, 96, false);
        let parsed = parse_dir_record(&rec).unwrap();
        assert_eq!(parsed.name, "SYSTEM.CNF;1");
        assert_eq!(parsed.extent_lba, 24);
        assert_eq!(parsed.size, 96);
        assert!(!parsed.is_dir);
    }

    #[test]
    fn name_matches_ignora_versao_e_caixa() {
        assert!(name_matches("SYSTEM.CNF;1", "system.cnf"));
        assert!(name_matches("SYSTEM.CNF", "SYSTEM.CNF"));
        assert!(!name_matches("SYSTEM.INI;1", "SYSTEM.CNF"));
    }

    #[test]
    fn parse_dir_records_varre_multiplos_e_para_no_padding() {
        let mut data = Vec::new();
        data.extend(make_record(".", 20, 2048, true));
        data.extend(make_record("..", 20, 2048, true));
        data.extend(make_record("SYSTEM.CNF;1", 24, 96, false));
        // resto do setor é zero (padding) — o varredor deve parar limpo.
        data.resize(SECTOR_SIZE, 0);

        let records = parse_dir_records(&data);
        assert_eq!(records.len(), 3);
        let sys = find_file(&records, "SYSTEM.CNF").unwrap();
        assert_eq!(sys.extent_lba, 24);
        assert_eq!(sys.size, 96);
    }

    #[test]
    fn find_file_ignora_diretorios_com_mesmo_nome() {
        let records = vec![
            DirRecord { name: "SYSTEM.CNF".into(), extent_lba: 1, size: 0, is_dir: true },
            DirRecord { name: "SYSTEM.CNF;1".into(), extent_lba: 2, size: 96, is_dir: false },
        ];
        assert_eq!(find_file(&records, "SYSTEM.CNF").unwrap().extent_lba, 2);
    }

    #[test]
    fn parse_root_record_valida_assinatura_cd001() {
        let mut pvd = vec![0u8; SECTOR_SIZE];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        let root = make_record(".", 20, 2048, true);
        pvd[156..156 + root.len()].copy_from_slice(&root);

        let parsed = parse_root_record(&pvd).unwrap();
        assert_eq!(parsed.extent_lba, 20);
        assert!(parsed.is_dir);

        // Assinatura errada → None.
        pvd[1] = b'X';
        assert!(parse_root_record(&pvd).is_none());
    }
}
