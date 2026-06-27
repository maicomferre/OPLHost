//! `ArtProvider` — baixa a capa (art) de um jogo por Game ID das fontes
//! externas (§7) e grava em `ART/` com a nomenclatura do OPL.
//!
//! Fonte padrão: o DB mensal do OPL Manager no archive.org, que permite extrair
//! UM arquivo de dentro do zip por URL (`.../<ITEM>.zip/PS2/<id>/<id>_COV.jpg`).
//! O archive.org retorna 503 intermitente nessa extração → o cliente HTTP real
//! faz retry/backoff. A `base URL` é configurável (mirror ou backup local).
//!
//! Desacoplamento/teste: a rede fica atrás do Trait [`HttpGet`]; a lógica de
//! nomes/URLs e o fluxo de `fetch_for_game` são testados com um mock, sem rede.

use std::path::Path;

use oplhost_core::GameId;

/// Item canônico atual do DB do OPL Manager no archive.org. A URL de extração
/// por arquivo aponta para dentro do zip: `<base>/PS2/<id>/<id>_<TIPO>.<ext>`.
pub const DEFAULT_BASE_URL: &str =
    "https://archive.org/download/OPLM_ART_2023_11/OPLM_ART_2023_11.zip";

/// Extensões tentadas, em ordem (a fonte mistura `.jpg` e `.png`).
const EXTENSIONS: [&str; 2] = ["jpg", "png"];

/// Tipos de art do OPL (sufixo do nome de arquivo). V1 prioriza `COV` (capa
/// frontal); os demais ficam disponíveis para fases futuras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtType {
    /// Capa frontal.
    Cov,
    /// Contracapa.
    Cov2,
    /// Ícone.
    Ico,
    /// Rótulo do disco.
    Lab,
    /// Logo.
    Lgo,
    /// Screenshot.
    Scr,
    /// Screenshot 2.
    Scr2,
    /// Fundo.
    Bg,
}

impl ArtType {
    /// Sufixo como gravado no nome do arquivo (ex.: `COV`).
    pub fn code(self) -> &'static str {
        match self {
            ArtType::Cov => "COV",
            ArtType::Cov2 => "COV2",
            ArtType::Ico => "ICO",
            ArtType::Lab => "LAB",
            ArtType::Lgo => "LGO",
            ArtType::Scr => "SCR",
            ArtType::Scr2 => "SCR2",
            ArtType::Bg => "BG",
        }
    }
}

/// Falha do `ArtProvider`. Mensagens descritivas para a UI orientar (§8); nunca
/// derruba o app — a UI mostra o texto e segue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtError {
    /// Falha ao gravar o arquivo de art no disco.
    Io(String),
    /// Falha de rede/HTTP após os retries (ex.: 503 persistente do archive.org).
    Http(String),
}

impl std::fmt::Display for ArtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtError::Io(m) => write!(f, "falha ao gravar art: {m}"),
            ArtError::Http(m) => write!(f, "falha ao baixar art: {m}"),
        }
    }
}

impl std::error::Error for ArtError {}

/// Resultado da busca de art de um jogo: o que foi baixado, o que já existia e o
/// que a fonte não tinha. Permite à UI relatar sem ambiguidade.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FetchOutcome {
    /// Nomes de arquivo recém-gravados em `ART/`.
    pub downloaded: Vec<String>,
    /// Arquivos que já existiam (não rebaixados — cache).
    pub skipped: Vec<String>,
    /// Tipos que a fonte não tinha (404 em todas as extensões).
    pub not_found: Vec<ArtType>,
}

/// Port de rede: um GET que devolve os bytes do corpo. `Ok(None)` = recurso
/// inexistente (404). `Err` só em falha real, já com retry/backoff aplicado pela
/// implementação real. Abstrai a rede para testar o `ArtProvider` sem ela.
pub trait HttpGet {
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>, ArtError>;
}

/// Monta o nome de destino em `ART/`: `<GameID>_<TIPO>.<ext>`.
pub fn art_dest_name(id: &GameId, ty: ArtType, ext: &str) -> String {
    format!("{}_{}.{ext}", id.as_str(), ty.code())
}

/// Monta a URL de extração por arquivo dentro do zip do archive.org.
pub fn art_url(base_url: &str, id: &GameId, ty: ArtType, ext: &str) -> String {
    let id = id.as_str();
    let base = base_url.trim_end_matches('/');
    format!("{base}/PS2/{id}/{id}_{}.{ext}", ty.code())
}

/// Baixa art por Game ID e grava em `ART/`. Genérico no cliente HTTP para
/// permitir mock nos testes.
pub struct ArtProvider<H: HttpGet> {
    http: H,
    base_url: String,
    types: Vec<ArtType>,
}

impl<H: HttpGet> ArtProvider<H> {
    /// Constrói com cliente, base URL e tipos explícitos (usado em teste e para
    /// apontar mirror/backup local).
    pub fn with_parts(http: H, base_url: String, types: Vec<ArtType>) -> Self {
        Self {
            http,
            base_url,
            types,
        }
    }

    /// Busca a art de um jogo, gravando em `art_dir` (a pasta `ART/` do destino).
    /// Para cada tipo, tenta `.jpg` e depois `.png`; o primeiro que existir é
    /// gravado. Com `overwrite = false`, arquivos já presentes são pulados
    /// (cache — não rebaixa). Falha de rede após retries vira `Err` (a UI mostra
    /// e segue, sem crash).
    pub fn fetch_for_game(
        &self,
        id: &GameId,
        art_dir: &Path,
        overwrite: bool,
    ) -> Result<FetchOutcome, ArtError> {
        let mut outcome = FetchOutcome::default();

        for &ty in &self.types {
            if let Some(name) = self.existing_art(id, ty, art_dir, overwrite) {
                outcome.skipped.push(name);
                continue;
            }

            let mut found = false;
            for ext in EXTENSIONS {
                let url = art_url(&self.base_url, id, ty, ext);
                match self.http.get(&url)? {
                    Some(bytes) => {
                        let name = art_dest_name(id, ty, ext);
                        std::fs::write(art_dir.join(&name), &bytes)
                            .map_err(|e| ArtError::Io(e.to_string()))?;
                        outcome.downloaded.push(name);
                        found = true;
                        break;
                    }
                    None => continue,
                }
            }
            if !found {
                outcome.not_found.push(ty);
            }
        }

        Ok(outcome)
    }

    /// Se já existe um arquivo de art desse tipo (qualquer extensão) e não é para
    /// sobrescrever, devolve o nome encontrado.
    fn existing_art(
        &self,
        id: &GameId,
        ty: ArtType,
        art_dir: &Path,
        overwrite: bool,
    ) -> Option<String> {
        if overwrite {
            return None;
        }
        EXTENSIONS.into_iter().find_map(|ext| {
            let name = art_dest_name(id, ty, ext);
            art_dir.join(&name).exists().then_some(name)
        })
    }
}

impl ArtProvider<UreqClient> {
    /// Configuração padrão: cliente `ureq` real, fonte do archive.org e apenas a
    /// capa frontal (`COV`) — o essencial da V1.
    pub fn new() -> Self {
        Self::with_parts(
            UreqClient::default(),
            DEFAULT_BASE_URL.to_string(),
            vec![ArtType::Cov],
        )
    }
}

impl Default for ArtProvider<UreqClient> {
    fn default() -> Self {
        Self::new()
    }
}

/// Cliente HTTP real sobre `ureq` (bloqueante, rustls). Faz retry com backoff em
/// respostas 502/503/504 — o archive.org devolve 503 intermitente na extração de
/// arquivo de dentro do zip.
pub struct UreqClient {
    max_retries: u32,
}

impl Default for UreqClient {
    fn default() -> Self {
        Self { max_retries: 3 }
    }
}

impl HttpGet for UreqClient {
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>, ArtError> {
        let mut attempt = 0;
        loop {
            match ureq::get(url).call() {
                Ok(mut resp) => {
                    let bytes = resp
                        .body_mut()
                        .read_to_vec()
                        .map_err(|e| ArtError::Http(e.to_string()))?;
                    return Ok(Some(bytes));
                }
                // Recurso inexistente: a fonte não tem essa art/extensão.
                Err(ureq::Error::StatusCode(404)) => return Ok(None),
                // Sobrecarga/rate-limit do archive.org: espera e tenta de novo.
                Err(ureq::Error::StatusCode(502..=504)) if attempt < self.max_retries => {
                    attempt += 1;
                    std::thread::sleep(backoff(attempt));
                }
                Err(e) => return Err(ArtError::Http(e.to_string())),
            }
        }
    }
}

/// Backoff exponencial simples: 500ms, 1s, 2s… limitado a 8s.
fn backoff(attempt: u32) -> std::time::Duration {
    let secs_ms = 500u64.saturating_mul(1 << (attempt.saturating_sub(1)).min(4));
    std::time::Duration::from_millis(secs_ms.min(8_000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Mock de `HttpGet`: serve bytes para URLs registradas, 404 (None) para o
    /// resto, e conta chamadas para checar a ordem jpg→png.
    struct MockHttp {
        responses: HashMap<String, Vec<u8>>,
        calls: RefCell<Vec<String>>,
    }

    impl MockHttp {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn serve(mut self, url: &str, bytes: &[u8]) -> Self {
            self.responses.insert(url.to_string(), bytes.to_vec());
            self
        }
    }

    impl HttpGet for MockHttp {
        fn get(&self, url: &str) -> Result<Option<Vec<u8>>, ArtError> {
            self.calls.borrow_mut().push(url.to_string());
            Ok(self.responses.get(url).cloned())
        }
    }

    fn temp_art_dir() -> std::path::PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let i = N.fetch_add(1, Ordering::Relaxed);
        let mut p = std::env::temp_dir();
        p.push(format!("oplhost-art-test-{}-{i}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn gid() -> GameId {
        GameId::parse("SCUS_973.13").unwrap()
    }

    #[test]
    fn monta_url_e_nome_no_padrao_do_oplm() {
        let id = gid();
        assert_eq!(
            art_url(DEFAULT_BASE_URL, &id, ArtType::Cov, "jpg"),
            "https://archive.org/download/OPLM_ART_2023_11/OPLM_ART_2023_11.zip\
             /PS2/SCUS_973.13/SCUS_973.13_COV.jpg"
        );
        assert_eq!(art_dest_name(&id, ArtType::Cov, "png"), "SCUS_973.13_COV.png");
    }

    #[test]
    fn url_nao_duplica_barra_da_base() {
        let id = gid();
        let with_slash = art_url("http://x/zip/", &id, ArtType::Cov, "jpg");
        assert_eq!(with_slash, "http://x/zip/PS2/SCUS_973.13/SCUS_973.13_COV.jpg");
    }

    #[test]
    fn baixa_cov_jpg_e_grava_em_art() {
        let id = gid();
        let dir = temp_art_dir();
        let url = art_url(DEFAULT_BASE_URL, &id, ArtType::Cov, "jpg");
        let http = MockHttp::new().serve(&url, b"JPGDATA");
        let provider = ArtProvider::with_parts(http, DEFAULT_BASE_URL.into(), vec![ArtType::Cov]);

        let out = provider.fetch_for_game(&id, &dir, false).unwrap();
        assert_eq!(out.downloaded, vec!["SCUS_973.13_COV.jpg"]);
        assert!(out.not_found.is_empty());
        let written = std::fs::read(dir.join("SCUS_973.13_COV.jpg")).unwrap();
        assert_eq!(written, b"JPGDATA");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cai_para_png_quando_jpg_nao_existe() {
        let id = gid();
        let dir = temp_art_dir();
        let png = art_url(DEFAULT_BASE_URL, &id, ArtType::Cov, "png");
        let http = MockHttp::new().serve(&png, b"PNGDATA");
        let provider = ArtProvider::with_parts(http, DEFAULT_BASE_URL.into(), vec![ArtType::Cov]);

        let out = provider.fetch_for_game(&id, &dir, false).unwrap();
        assert_eq!(out.downloaded, vec!["SCUS_973.13_COV.png"]);
        assert!(dir.join("SCUS_973.13_COV.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nao_rebaixa_o_que_ja_existe() {
        let id = gid();
        let dir = temp_art_dir();
        std::fs::write(dir.join("SCUS_973.13_COV.jpg"), b"OLD").unwrap();
        // Sem URLs servidas: se tentasse baixar, viraria not_found.
        let http = MockHttp::new();
        let provider = ArtProvider::with_parts(http, DEFAULT_BASE_URL.into(), vec![ArtType::Cov]);

        let out = provider.fetch_for_game(&id, &dir, false).unwrap();
        assert_eq!(out.skipped, vec!["SCUS_973.13_COV.jpg"]);
        assert!(out.downloaded.is_empty());
        // O arquivo antigo permanece intacto.
        assert_eq!(std::fs::read(dir.join("SCUS_973.13_COV.jpg")).unwrap(), b"OLD");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ausencia_na_fonte_vira_not_found_sem_erro() {
        let id = gid();
        let dir = temp_art_dir();
        let http = MockHttp::new(); // 404 para tudo
        let provider = ArtProvider::with_parts(http, DEFAULT_BASE_URL.into(), vec![ArtType::Cov]);

        let out = provider.fetch_for_game(&id, &dir, false).unwrap();
        assert_eq!(out.not_found, vec![ArtType::Cov]);
        assert!(out.downloaded.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
