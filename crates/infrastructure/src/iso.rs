//! Leitor de ISO de PS2: extrai o Game ID lendo o `SYSTEM.CNF`. Faz só o I/O
//! (seek/read dos setores necessários) e delega TODO o parse ao `core`
//! (`iso9660` + `parse_boot2_game_id`). Lê apenas o PVD, o diretório raiz e o
//! `SYSTEM.CNF` — nunca a ISO inteira (§4: não mover arquivos grandes à toa).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use oplhost_core::GameId;
use oplhost_core::game_id::parse_boot2_game_id;
use oplhost_core::iso9660::{
    PVD_LBA, SECTOR_SIZE, find_file, parse_dir_records, parse_root_record,
};

/// Lê o Game ID de uma ISO de PS2. `Ok(None)` quando o arquivo não é uma ISO9660
/// reconhecível ou não tem `SYSTEM.CNF`/`BOOT2` válido — a UI trata como
/// "desconhecido" sem erro. `Err` só em falha real de I/O.
pub fn read_game_id(path: &Path) -> std::io::Result<Option<GameId>> {
    let mut file = File::open(path)?;

    let pvd = read_extent(&mut file, PVD_LBA, SECTOR_SIZE as u32)?;
    let root = match parse_root_record(&pvd) {
        Some(r) => r,
        None => return Ok(None),
    };

    let root_data = read_extent(&mut file, root.extent_lba, root.size)?;
    let records = parse_dir_records(&root_data);
    let sys = match find_file(&records, "SYSTEM.CNF") {
        Some(s) => s,
        None => return Ok(None),
    };

    let cnf = read_extent(&mut file, sys.extent_lba, sys.size)?;
    let text = String::from_utf8_lossy(&cnf);
    Ok(parse_boot2_game_id(&text))
}

/// Lê `size` bytes a partir do setor `lba` (offset = `lba * SECTOR_SIZE`).
fn read_extent(file: &mut File, lba: u32, size: u32) -> std::io::Result<Vec<u8>> {
    let offset = lba as u64 * SECTOR_SIZE as u64;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; size as usize];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::unique_path;
    use std::io::Write;

    /// Monta um registro de diretório ISO9660 sintético.
    fn dir_record(name: &str, extent: u32, size: u32, is_dir: bool) -> Vec<u8> {
        let nb = name.as_bytes();
        let len = 33 + nb.len();
        let mut rec = vec![0u8; len];
        rec[0] = len as u8;
        rec[2..6].copy_from_slice(&extent.to_le_bytes());
        rec[10..14].copy_from_slice(&size.to_le_bytes());
        rec[25] = if is_dir { 0x02 } else { 0x00 };
        rec[32] = nb.len() as u8;
        rec[33..].copy_from_slice(nb);
        rec
    }

    fn temp_iso_path() -> std::path::PathBuf {
        unique_path("iso").with_extension("iso")
    }

    /// Constrói uma ISO mínima válida: PVD no setor 16 apontando o diretório raiz
    /// no setor 17, que lista `SYSTEM.CNF` no setor 18 com um `BOOT2`.
    fn build_minimal_iso(boot2_id: &str) -> std::path::PathBuf {
        let mut img = vec![0u8; 19 * SECTOR_SIZE];

        // PVD @ setor 16.
        let pvd = 16 * SECTOR_SIZE;
        img[pvd] = 1;
        img[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        let root = dir_record(".", 17, SECTOR_SIZE as u32, true);
        img[pvd + 156..pvd + 156 + root.len()].copy_from_slice(&root);

        // Diretório raiz @ setor 17.
        let dir = 17 * SECTOR_SIZE;
        let cnf_rec = dir_record("SYSTEM.CNF;1", 18, 64, false);
        img[dir..dir + cnf_rec.len()].copy_from_slice(&cnf_rec);

        // SYSTEM.CNF @ setor 18.
        let cnf = 18 * SECTOR_SIZE;
        let body = format!("BOOT2 = cdrom0:\\{boot2_id};1\nVER = 1.00\n");
        img[cnf..cnf + body.len()].copy_from_slice(body.as_bytes());

        let path = temp_iso_path();
        let mut f = File::create(&path).unwrap();
        f.write_all(&img).unwrap();
        path
    }

    #[test]
    fn extrai_game_id_de_iso_minima() {
        let path = build_minimal_iso("SLUS_213.86");
        let id = read_game_id(&path).unwrap();
        assert_eq!(id.unwrap().as_str(), "SLUS_213.86");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn arquivo_sem_assinatura_iso_vira_none() {
        let path = temp_iso_path();
        std::fs::write(&path, vec![0u8; 17 * SECTOR_SIZE]).unwrap();
        assert_eq!(read_game_id(&path).unwrap(), None);
        let _ = std::fs::remove_file(&path);
    }
}
