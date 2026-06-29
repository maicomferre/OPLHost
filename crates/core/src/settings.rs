//! `config.json` — configuração de UI DA NOSSA aplicação (não do OPL).
//!
//! Estado **não-sensível** que sobrevive entre execuções: último diretório-alvo,
//! se o acesso autenticado estava marcado e o usuário do share. Vive em
//! `$XDG_CONFIG_HOME/oplhost/config.json` (config pertence ao usuário, não ao
//! disco-alvo — por isso fica fora do `opl_meta.json`, §6).
//!
//! **Regra inegociável:** a senha do share **nunca** é gravada aqui — ela vive
//! no Samba do sistema (`passdb.tdb`). `AppSettings` não tem campo de senha.
//!
//! Como o `opl_meta.json` (§6), é conveniência e nunca fonte de verdade: config
//! ausente ou corrompida cai em `default()` sem derrubar o app. A lógica de
//! tolerância fica no adapter (`FsSettingsStore`), igual ao `MetaStore`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Versão do schema do `config.json`. Permite migração futura sem quebrar
/// arquivos antigos — campos novos entram com `#[serde(default)]`.
pub const SETTINGS_VERSION: u32 = 1;

/// Estado de UI persistido entre execuções. Só campos **não-sensíveis**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub version: u32,
    /// Último diretório-alvo escolhido. Pré-preenche o campo no próximo start.
    #[serde(default)]
    pub last_target_dir: Option<PathBuf>,
    /// Se o acesso autenticado (toggle) estava marcado.
    #[serde(default)]
    pub auth_required: bool,
    /// Usuário do share (= dono da pasta). Guardado por completude; o app o
    /// deriva da conta do sistema de qualquer forma. **Nunca** acompanha senha.
    #[serde(default)]
    pub auth_username: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            last_target_dir: None,
            auth_required: false,
            auth_username: None,
        }
    }
}

/// Port de persistência da config de UI. Implementado na infraestrutura
/// (`FsSettingsStore`); mockável nos testes do `core`. Mesmo desenho do
/// [`crate::MetaStore`], mas `load` não falha: config ausente/corrompida vira
/// `default()` (a config é conveniência, jamais obrigatória).
pub trait SettingsStore {
    /// Lê a config do usuário, caindo em `default()` se ausente ou corrompida.
    fn load(&self) -> AppSettings;
    /// Grava a config do usuário. Best-effort na UI — falha não derruba o app.
    fn save(&self, settings: &AppSettings) -> Result<(), SettingsError>;
}

/// Falha de escrita da config. A leitura nunca reporta erro (cai em default);
/// só a escrita pode falhar, e ainda assim a UI segue (save é best-effort).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// Falha de I/O ao criar o diretório ou gravar o arquivo.
    Io(String),
    /// Falha ao serializar a config para JSON.
    Serialize(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::Io(m) => write!(f, "falha de I/O no config.json: {m}"),
            SettingsError::Serialize(m) => write!(f, "config.json não serializável: {m}"),
        }
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preenche_versao_e_zera_campos() {
        let s = AppSettings::default();
        assert_eq!(s.version, SETTINGS_VERSION);
        assert_eq!(s.last_target_dir, None);
        assert!(!s.auth_required);
        assert_eq!(s.auth_username, None);
    }

    #[test]
    fn roundtrip_json_preserva_a_config() {
        let s = AppSettings {
            version: SETTINGS_VERSION,
            last_target_dir: Some(PathBuf::from("/mnt/ps2")),
            auth_required: true,
            auth_username: Some("maicom".to_string()),
        };
        let json = serde_json::to_string(&s).unwrap();
        let voltou: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, voltou);
    }

    #[test]
    fn config_sem_campos_novos_ainda_carrega() {
        // Um config.json mínimo (só version) deve carregar — campos ausentes
        // caem no default via #[serde(default)], honrando o §6.
        let json = r#"{"version":1}"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.last_target_dir, None);
        assert!(!s.auth_required);
        assert_eq!(s.auth_username, None);
    }

    /// Garantia de segurança: o JSON serializado NUNCA contém um campo de senha.
    /// Se alguém acrescentar `password` ao struct, este teste quebra de propósito.
    #[test]
    fn config_serializada_jamais_contem_senha() {
        let s = AppSettings {
            auth_required: true,
            auth_username: Some("maicom".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap().to_lowercase();
        assert!(!json.contains("password"));
        assert!(!json.contains("senha"));
        assert!(!json.contains("passwd"));
    }
}
