//! Editor de metadados do jogo — os campos que o OPL exibe na tela "Informações".
//!
//! **Onde o OPL persiste (validado na fonte `ps2homebrew/Open-PS2-Loader`):**
//! `CFG/<GameID>.cfg` na raiz do dispositivo (`src/opl.c`). Formato INI
//! `chave=valor` (`src/config.c`). O theme engine lê os campos por nome de
//! atributo (`src/themes.c`); os 5 rotulados nativamente (`lng_tmpl/_base.yml`)
//! são `Title`, `Genre`, `Release`, `Developer`, `Description`.
//!
//! **Regra crítica:** o MESMO `.cfg` guarda as configs de compatibilidade do OPL
//! (`$Compatibility`, `$VMC_0`, …, com prefixo `$`). Gravar nossos campos é
//! **read-modify-write**: [`GameCfg`] preserva toda chave desconhecida e a ordem;
//! [`GameCfg::apply_info`] mexe só nos 5 campos de info. Nunca reescrever o
//! arquivo inteiro — isso apagaria os ajustes de compatibilidade do usuário.
//!
//! Este módulo é PURO (parse/serialize/validação); o I/O em `CFG/<id>.cfg` mora
//! no adapter `FsGameInfoStore` da infraestrutura (inversão do §3, como o
//! `MetaStore`/`JsonMetaStore`).

use crate::game_id::GameId;

/// Chave do OPL para o título exibido (sobrescreve o nome derivado do arquivo).
pub const KEY_TITLE: &str = "Title";
/// Chave do OPL para o gênero.
pub const KEY_GENRE: &str = "Genre";
/// Chave do OPL para a data/ano de lançamento (texto livre exibido como veio).
pub const KEY_RELEASE: &str = "Release";
/// Chave do OPL para o desenvolvedor/publicadora.
pub const KEY_DEVELOPER: &str = "Developer";
/// Chave do OPL para a descrição.
pub const KEY_DESCRIPTION: &str = "Description";

/// Tamanho máximo de um valor no `.cfg` do OPL. O parser do OPL usa um buffer de
/// `CONFIG_KEY_VALUE_LEN = 256` (`include/config.h`), então o valor útil é 255.
pub const OPL_VALUE_MAX_LEN: usize = 255;

/// Metadados editáveis de um jogo (tela "Informações" do OPL). Cada campo é
/// opcional: `None` = chave ausente no `.cfg` (OPL cai no padrão; p/ o título,
/// usa o nome derivado do arquivo). String vazia nunca é gravada — vira `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameInfo {
    pub title: Option<String>,
    pub genre: Option<String>,
    pub release: Option<String>,
    pub developer: Option<String>,
    pub description: Option<String>,
}

impl GameInfo {
    /// `true` se nenhum campo está preenchido (nada a gravar / info "vazia").
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.genre.is_none()
            && self.release.is_none()
            && self.developer.is_none()
            && self.description.is_none()
    }

    /// Valida os campos contra os limites do OPL: nenhum valor pode passar de
    /// [`OPL_VALUE_MAX_LEN`] nem conter quebra de linha (cada valor é 1 linha no
    /// `.cfg`). Retorna a lista de erros por campo (vazia = válido), para a UI
    /// destacar exatamente o que corrigir.
    pub fn validate(&self) -> Vec<FieldError> {
        let mut errs = Vec::new();
        for (key, value) in self.fields() {
            let Some(v) = value else { continue };
            if v.contains('\n') || v.contains('\r') {
                errs.push(FieldError {
                    key,
                    kind: FieldErrorKind::Newline,
                });
            }
            if v.chars().count() > OPL_VALUE_MAX_LEN {
                errs.push(FieldError {
                    key,
                    kind: FieldErrorKind::TooLong {
                        len: v.chars().count(),
                    },
                });
            }
        }
        errs
    }

    /// Pares `(chave_opl, valor)` na ordem do OPL. Interno ao módulo.
    fn fields(&self) -> [(&'static str, Option<&str>); 5] {
        [
            (KEY_TITLE, self.title.as_deref()),
            (KEY_GENRE, self.genre.as_deref()),
            (KEY_RELEASE, self.release.as_deref()),
            (KEY_DEVELOPER, self.developer.as_deref()),
            (KEY_DESCRIPTION, self.description.as_deref()),
        ]
    }
}

/// Erro de validação de um campo, ligado à chave do OPL correspondente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub key: &'static str,
    pub kind: FieldErrorKind,
}

/// Natureza do erro de um campo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldErrorKind {
    /// Valor excede o limite do OPL.
    TooLong { len: usize },
    /// Valor contém quebra de linha (inválido num `.cfg` de 1 linha por chave).
    Newline,
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FieldErrorKind::TooLong { len } => write!(
                f,
                "{}: {len} caracteres (máximo {OPL_VALUE_MAX_LEN})",
                self.key
            ),
            FieldErrorKind::Newline => {
                write!(f, "{}: não pode conter quebra de linha", self.key)
            }
        }
    }
}

/// Erro de normalização do campo Lançamento: o usuário digitou algo que *parece*
/// uma data (só dígitos e separadores) mas não é válida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseError(pub String);

impl std::fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ReleaseError {}

/// Dias no mês, ciente de ano bissexto. Usado para validar a data do Lançamento.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap =
                (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
}

/// Normaliza/valida o campo **Lançamento** (`Release`).
///
/// O OPL **não** interpreta esse campo como data — exibe o texto verbatim
/// (`src/themes.c`). Então não há formato obrigatório do ponto de vista do OPL.
/// Mesmo assim, sanitizamos para o usuário não gravar uma "data" sem sentido:
///
/// - Vazio → `Ok(None)` (remove a chave).
/// - Contém letra (ex.: "Verão de 2007") → texto livre, devolvido como veio
///   (o OPL aceita; não temos como adivinhar uma data ali).
/// - Só dígitos e separadores → interpretado como data e **canonizado**:
///   `AAAA`, `AAAA-MM` ou `AAAA-MM-DD` (zero-padded). Aceita ordem
///   `AAAA-MM-DD` ou `DD/MM/AAAA` (decidida por qual extremo tem 4 dígitos).
///   Data fora de faixa (mês > 12, dia inexistente) → `Err`. Ano de 2 dígitos
///   (ambíguo) → `Err`, pedindo o ano com 4 dígitos.
pub fn normalize_release(raw: &str) -> Result<Option<String>, ReleaseError> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    // Tem letra → texto livre; o OPL mostra como veio.
    if t.chars().any(char::is_alphabetic) {
        return Ok(Some(t.to_string()));
    }

    let invalid = || {
        Err(ReleaseError(format!(
            "\"{t}\" não é uma data válida. Use AAAA-MM-DD, DD/MM/AAAA ou só o ano \
             (ou escreva por extenso)."
        )))
    };

    // Só dígitos e separadores: quebra nos não-dígitos.
    let parts: Vec<&str> = t
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect();
    let nums: Vec<u32> = parts.iter().filter_map(|p| p.parse().ok()).collect();
    if nums.len() != parts.len() {
        return invalid(); // número grande demais p/ u32 etc.
    }

    let valid_year = |y: u32| (1000..=2999).contains(&y);

    match (parts.as_slice(), nums.as_slice()) {
        // Só o ano.
        ([y], [year]) if y.len() == 4 && valid_year(*year) => Ok(Some(year.to_string())),
        // Ano-mês, em qualquer ordem (o de 4 dígitos é o ano).
        ([a, b], [na, nb]) => {
            let (year, month) = if a.len() == 4 {
                (*na, *nb)
            } else if b.len() == 4 {
                (*nb, *na)
            } else {
                return invalid();
            };
            if valid_year(year) && (1..=12).contains(&month) {
                Ok(Some(format!("{year:04}-{month:02}")))
            } else {
                invalid()
            }
        }
        // Ano-mês-dia: AAAA-MM-DD ou DD-MM-AAAA (4 dígitos num dos extremos).
        ([a, _, c], [na, nb, nc]) => {
            let (year, month, day) = if a.len() == 4 {
                (*na, *nb, *nc)
            } else if c.len() == 4 {
                (*nc, *nb, *na)
            } else {
                return invalid();
            };
            if valid_year(year)
                && (1..=12).contains(&month)
                && (1..=days_in_month(year, month)).contains(&day)
            {
                Ok(Some(format!("{year:04}-{month:02}-{day:02}")))
            } else {
                invalid()
            }
        }
        _ => invalid(),
    }
}

/// Uma linha do `.cfg`: ou um par `chave=valor`, ou uma linha "crua" (em branco
/// ou que não casou `=`) preservada tal qual para round-trip fiel.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CfgLine {
    Pair { key: String, value: String },
    Raw(String),
}

/// Representação em memória de um `CFG/<id>.cfg`, preservando ordem e linhas
/// desconhecidas. É a peça que garante o read-modify-write: ao gravar info, só os
/// 5 campos mudam; tudo o mais (compatibilidade `$…`, comentários, ordem) fica.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameCfg {
    lines: Vec<CfgLine>,
}

impl GameCfg {
    /// Lê um `.cfg` cru. Cada linha vira `Pair` (se tiver `=`) ou `Raw`. O OPL
    /// separa no primeiro `=` e ignora whitespace inicial da chave
    /// (`src/config.c`); espelhamos isso para que `get`/`apply` casem o que o
    /// OPL casaria. Linhas sem `=` viram `Raw` (preservadas, não perdidas).
    pub fn parse(raw: &str) -> GameCfg {
        let mut lines = Vec::new();
        for line in raw.lines() {
            match line.split_once('=') {
                Some((k, v)) => lines.push(CfgLine::Pair {
                    key: k.trim().to_string(),
                    value: v.to_string(),
                }),
                None => lines.push(CfgLine::Raw(line.to_string())),
            }
        }
        GameCfg { lines }
    }

    /// Valor de uma chave (primeira ocorrência), se presente.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| match l {
            CfgLine::Pair { key: k, value } if k == key => Some(value.as_str()),
            _ => None,
        })
    }

    /// Define/atualiza uma chave **no lugar** (preserva a posição da 1ª
    /// ocorrência); se ausente, acrescenta ao fim. Não duplica.
    pub fn set(&mut self, key: &str, value: &str) {
        for l in &mut self.lines {
            if let CfgLine::Pair { key: k, value: v } = l
                && k == key
            {
                *v = value.to_string();
                return;
            }
        }
        self.lines.push(CfgLine::Pair {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    /// Remove todas as ocorrências de uma chave. Outras linhas ficam intactas.
    pub fn remove(&mut self, key: &str) {
        self.lines
            .retain(|l| !matches!(l, CfgLine::Pair { key: k, .. } if k == key));
    }

    /// Extrai os 5 campos de info do `.cfg` atual.
    pub fn info(&self) -> GameInfo {
        let read = |key: &str| self.get(key).map(str::to_string);
        GameInfo {
            title: read(KEY_TITLE),
            genre: read(KEY_GENRE),
            release: read(KEY_RELEASE),
            developer: read(KEY_DEVELOPER),
            description: read(KEY_DESCRIPTION),
        }
    }

    /// Aplica os 5 campos de info **preservando o resto** (read-modify-write):
    /// campo `Some` grava/atualiza a chave; campo `None` remove. Chaves de
    /// compatibilidade (`$…`) e quaisquer outras nunca são tocadas.
    pub fn apply_info(&mut self, info: &GameInfo) {
        for (key, value) in info.fields() {
            match value {
                Some(v) => self.set(key, v),
                None => self.remove(key),
            }
        }
    }
}

impl std::fmt::Display for GameCfg {
    /// Serializa de volta ao formato `chave=valor`, uma linha por entrada,
    /// terminando com `\n` (convenção de arquivo de texto Unix).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.lines {
            match line {
                CfgLine::Pair { key, value } => writeln!(f, "{key}={value}")?,
                CfgLine::Raw(raw) => writeln!(f, "{raw}")?,
            }
        }
        Ok(())
    }
}

/// Nome do arquivo de config do jogo lido pelo OPL: `<GameID>.cfg`
/// (ex.: `SLUS_200.02.cfg`). Centraliza a convenção num lugar só.
pub fn cfg_file_name(game_id: &GameId) -> String {
    format!("{}.cfg", game_id.as_str())
}

/// Port de leitura/escrita do info do jogo. Implementado na infraestrutura
/// (`FsGameInfoStore`); mockável nos testes do `core`. O `save` é
/// read-modify-write (preserva compatibilidade) — contrato garantido pelo
/// adapter usando [`GameCfg`].
pub trait GameInfoStore {
    /// Lê o info atual do jogo. `.cfg` ausente → [`GameInfo::default`] (vazio).
    fn load(&self, game_id: &GameId) -> Result<GameInfo, GameInfoError>;
    /// Grava o info preservando as demais chaves do `.cfg`.
    fn save(&self, game_id: &GameId, info: &GameInfo) -> Result<(), GameInfoError>;
}

/// Falha de leitura/escrita do info. Erro de info nunca deve derrubar o app — a
/// UI relata e segue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameInfoError {
    /// Falha de I/O ao ler/gravar o `.cfg`.
    Io(String),
    /// Campo inválido (passa do limite do OPL ou tem quebra de linha). Carrega a
    /// lista de erros por campo para a UI exibir.
    Invalid(Vec<FieldError>),
}

impl std::fmt::Display for GameInfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameInfoError::Io(m) => write!(f, "falha de I/O no .cfg do jogo: {m}"),
            GameInfoError::Invalid(errs) => {
                write!(f, "campos inválidos: ")?;
                for (i, e) in errs.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for GameInfoError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_e_info_extraem_os_cinco_campos() {
        let raw = "Title=God of War\nGenre=Action\nRelease=2005-03-22\n\
                   Developer=SCE Santa Monica\nDescription=Kratos vs gods.\n";
        let info = GameCfg::parse(raw).info();
        assert_eq!(info.title.as_deref(), Some("God of War"));
        assert_eq!(info.genre.as_deref(), Some("Action"));
        assert_eq!(info.release.as_deref(), Some("2005-03-22"));
        assert_eq!(info.developer.as_deref(), Some("SCE Santa Monica"));
        assert_eq!(info.description.as_deref(), Some("Kratos vs gods."));
    }

    #[test]
    fn info_de_cfg_vazio_e_default() {
        assert_eq!(GameCfg::default().info(), GameInfo::default());
        assert!(GameCfg::parse("").info().is_empty());
    }

    /// TRAVA: o `.cfg` guarda configs de compatibilidade no mesmo arquivo.
    /// `apply_info` jamais pode apagá-las (read-modify-write).
    #[test]
    fn apply_info_preserva_chaves_de_compatibilidade() {
        let raw = "$Compatibility=4\n$VMC_0=Game\nGenre=OldGenre\n";
        let mut cfg = GameCfg::parse(raw);
        cfg.apply_info(&GameInfo {
            title: Some("Novo Título".into()),
            genre: Some("RPG".into()),
            ..Default::default()
        });
        // Compatibilidade intacta...
        assert_eq!(cfg.get("$Compatibility"), Some("4"));
        assert_eq!(cfg.get("$VMC_0"), Some("Game"));
        // ...info atualizado/inserido.
        assert_eq!(cfg.get(KEY_GENRE), Some("RPG"));
        assert_eq!(cfg.get(KEY_TITLE), Some("Novo Título"));
    }

    #[test]
    fn set_atualiza_no_lugar_sem_duplicar() {
        let mut cfg = GameCfg::parse("Genre=Action\n$Compatibility=1\n");
        cfg.set(KEY_GENRE, "Adventure");
        // Atualizou no lugar: ainda uma só linha de Genre, antes do $Compatibility.
        let out = cfg.to_string();
        assert_eq!(out, "Genre=Adventure\n$Compatibility=1\n");
        assert_eq!(out.matches("Genre=").count(), 1);
    }

    #[test]
    fn apply_info_com_campo_none_remove_a_chave() {
        let mut cfg = GameCfg::parse("Title=Antigo\nGenre=Action\n$VMC_1=X\n");
        cfg.apply_info(&GameInfo {
            genre: Some("Racing".into()),
            ..Default::default() // title=None → remove
        });
        assert_eq!(cfg.get(KEY_TITLE), None);
        assert_eq!(cfg.get(KEY_GENRE), Some("Racing"));
        assert_eq!(cfg.get("$VMC_1"), Some("X"));
    }

    #[test]
    fn parse_preserva_linhas_desconhecidas_e_ordem_no_roundtrip() {
        let raw = "$Compatibility=2\nGenre=Action\nLinhaEstranhaSemIgual\n";
        let cfg = GameCfg::parse(raw);
        assert_eq!(cfg.to_string(), raw);
    }

    #[test]
    fn parse_ignora_whitespace_inicial_da_chave_como_o_opl() {
        // O OPL ignora whitespace à esquerda da chave; o get deve casar.
        let cfg = GameCfg::parse("    Genre=Action\n");
        assert_eq!(cfg.get(KEY_GENRE), Some("Action"));
    }

    #[test]
    fn validate_reprova_valor_longo_demais() {
        let info = GameInfo {
            description: Some("a".repeat(OPL_VALUE_MAX_LEN + 1)),
            ..Default::default()
        };
        let errs = info.validate();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].key, KEY_DESCRIPTION);
        assert!(matches!(errs[0].kind, FieldErrorKind::TooLong { .. }));
    }

    #[test]
    fn validate_aceita_valor_no_limite() {
        let info = GameInfo {
            description: Some("a".repeat(OPL_VALUE_MAX_LEN)),
            ..Default::default()
        };
        assert!(info.validate().is_empty());
    }

    #[test]
    fn validate_reprova_quebra_de_linha() {
        let info = GameInfo {
            genre: Some("Action\nAdventure".into()),
            ..Default::default()
        };
        let errs = info.validate();
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0].kind, FieldErrorKind::Newline));
    }

    #[test]
    fn cfg_file_name_usa_o_game_id() {
        let id = GameId::parse("SLUS_200.02").unwrap();
        assert_eq!(cfg_file_name(&id), "SLUS_200.02.cfg");
    }

    #[test]
    fn valor_pode_conter_igual_apos_o_primeiro() {
        // splitn no 1º '=': "a=b=c" → chave "a", valor "b=c".
        let cfg = GameCfg::parse("Description=x = y = z\n");
        assert_eq!(cfg.get(KEY_DESCRIPTION), Some("x = y = z"));
    }

    #[test]
    fn release_vazio_vira_none() {
        assert_eq!(normalize_release("   "), Ok(None));
    }

    #[test]
    fn release_canonico_passa_e_zera_pad() {
        assert_eq!(
            normalize_release("2007-3-1"),
            Ok(Some("2007-03-01".to_string()))
        );
        assert_eq!(
            normalize_release("2005-03-22"),
            Ok(Some("2005-03-22".to_string()))
        );
    }

    #[test]
    fn release_formato_br_e_reordenado_para_iso() {
        assert_eq!(
            normalize_release("24/06/2007"),
            Ok(Some("2007-06-24".to_string()))
        );
    }

    #[test]
    fn release_so_ano_ou_ano_mes() {
        assert_eq!(normalize_release("2007"), Ok(Some("2007".to_string())));
        assert_eq!(
            normalize_release("03/2007"),
            Ok(Some("2007-03".to_string()))
        );
    }

    #[test]
    fn release_data_like_invalida_e_rejeitada() {
        // O caso do usuário: 2007-03-132.
        assert!(normalize_release("2007-03-132").is_err());
        // Mês inexistente.
        assert!(normalize_release("2007-13-01").is_err());
        // 29 de fev em ano não-bissexto.
        assert!(normalize_release("2007-02-29").is_err());
        // Ano de 2 dígitos: ambíguo → erro pedindo 4 dígitos.
        assert!(normalize_release("12/06/07").is_err());
    }

    #[test]
    fn release_29_fev_bissexto_passa() {
        assert_eq!(
            normalize_release("2008-02-29"),
            Ok(Some("2008-02-29".to_string()))
        );
    }

    #[test]
    fn release_texto_livre_com_letras_passa_verbatim() {
        // OPL exibe verbatim; não tentamos adivinhar uma data.
        assert_eq!(
            normalize_release("Verão de 2007"),
            Ok(Some("Verão de 2007".to_string()))
        );
    }
}
