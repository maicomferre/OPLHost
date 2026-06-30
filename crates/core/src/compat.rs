//! Modos de compatibilidade do OPL por jogo — a tela "Game Settings".
//!
//! **Onde o OPL persiste (validado na fonte `ps2homebrew/Open-PS2-Loader`):**
//! a chave `$Compatibility` (`CONFIG_ITEM_COMPAT`, `include/config.h`) **no mesmo**
//! `CFG/<GameID>.cfg` que os campos de info. O valor é um **bitmask decimal cru**,
//! sem bit de "configurado"/offset: o OPL lê com `configGetInt(&compatmask)` e
//! testa direto `if (compatmask & COMPAT_MODE_n)` (`src/supportbase.c`).
//!
//! **Bits** (`include/iosupport.h`, `COMPAT_MODE_COUNT = 6`):
//! `0x01` Accurate Reads, `0x02` Synchronous Mode, `0x04` Unhook Syscalls,
//! `0x08` Skip Videos, `0x10` Emulate DVD-DL, `0x20` Disable IGR. Os bits `0x40`/
//! `0x80` (Modes 7/8) existem mas estão sem uso/rótulo no OPL base — nós só os
//! **preservamos** (ver [`CompatFlags`]), nunca os expomos para marcar.
//!
//! Módulo PURO: o I/O em `CFG/<id>.cfg` mora no adapter `FsGameInfoStore`, que
//! grava por read-modify-write usando [`crate::GameCfg`] (preserva info e demais
//! chaves `$`). Os rótulos aqui são o idioma-fonte (en); a UI traduz via `@tr`.

use crate::game_info::GameCfg;

/// Chave do OPL para o bitmask de compatibilidade (mesmo `.cfg` do info).
pub const CONFIG_ITEM_COMPAT: &str = "$Compatibility";

/// Os 6 modos de compatibilidade em uso no OPL base (`COMPAT_MODE_COUNT = 6`).
/// Modes 7/8 existem como bits mas não têm uso/rótulo — não entram aqui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatMode {
    /// `0x01` — leitura precisa de disco (mais lenta, mais compatível).
    AccurateReads,
    /// `0x02` — modo de leitura síncrono (alternativo).
    SynchronousMode,
    /// `0x04` — desfaz hooks de syscalls.
    UnhookSyscalls,
    /// `0x08` — pula vídeos (PSS de tamanho 0 e Bink).
    SkipVideos,
    /// `0x10` — emula DVD dual-layer.
    EmulateDvdDl,
    /// `0x20` — desabilita o In-Game Reset.
    DisableIgr,
}

impl CompatMode {
    /// Todos os modos em uso, em ordem (Mode 1 → Mode 6).
    pub const ALL: [CompatMode; 6] = [
        CompatMode::AccurateReads,
        CompatMode::SynchronousMode,
        CompatMode::UnhookSyscalls,
        CompatMode::SkipVideos,
        CompatMode::EmulateDvdDl,
        CompatMode::DisableIgr,
    ];

    /// O bit do modo no bitmask `$Compatibility` (`COMPAT_MODE_n`).
    pub fn bit(self) -> u8 {
        match self {
            CompatMode::AccurateReads => 0x01,
            CompatMode::SynchronousMode => 0x02,
            CompatMode::UnhookSyscalls => 0x04,
            CompatMode::SkipVideos => 0x08,
            CompatMode::EmulateDvdDl => 0x10,
            CompatMode::DisableIgr => 0x20,
        }
    }

    /// Número do modo como o OPL exibe (1–6).
    pub fn number(self) -> u8 {
        match self {
            CompatMode::AccurateReads => 1,
            CompatMode::SynchronousMode => 2,
            CompatMode::UnhookSyscalls => 3,
            CompatMode::SkipVideos => 4,
            CompatMode::EmulateDvdDl => 5,
            CompatMode::DisableIgr => 6,
        }
    }

    /// Rótulo nativo do OPL (idioma-fonte en, de `lng_tmpl/_base.yml`). A UI deve
    /// traduzir via `@tr`; este valor é referência/teste e fallback.
    pub fn label(self) -> &'static str {
        match self {
            CompatMode::AccurateReads => "Accurate Reads",
            CompatMode::SynchronousMode => "Synchronous Mode",
            CompatMode::UnhookSyscalls => "Unhook Syscalls",
            CompatMode::SkipVideos => "Skip Videos",
            CompatMode::EmulateDvdDl => "Emulate DVD-DL",
            CompatMode::DisableIgr => "Disable IGR",
        }
    }
}

/// Bitmask de compatibilidade de um jogo. Envelopa o `u8` **inteiro** do
/// `$Compatibility`, então bits que a UI não expõe (Modes 7/8) são preservados
/// num round-trip — read-modify-write também no nível de bit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompatFlags(u8);

impl CompatFlags {
    /// Lê o valor cru da chave `$Compatibility`. O OPL usa `configGetInt`
    /// (decimal); valor ausente, vazio ou não-numérico → sem modos (`0`).
    pub fn parse(raw: &str) -> CompatFlags {
        CompatFlags(raw.trim().parse::<u8>().unwrap_or(0))
    }

    /// Valor a gravar na chave, ou `None` quando nenhum bit está setado — aí a
    /// chave é **removida** (mesma regra de "campo vazio" do info), deixando o
    /// `.cfg` limpo e o OPL no padrão.
    pub fn to_config_value(self) -> Option<String> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0.to_string())
        }
    }

    /// `true` se o modo está marcado.
    pub fn is_set(self, mode: CompatMode) -> bool {
        self.0 & mode.bit() != 0
    }

    /// Marca/desmarca um modo (preserva os demais bits, inclusive os 7/8).
    pub fn set(&mut self, mode: CompatMode, on: bool) {
        if on {
            self.0 |= mode.bit();
        } else {
            self.0 &= !mode.bit();
        }
    }

    /// `true` se nenhum bit está setado (nada a gravar).
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl GameCfg {
    /// Extrai o bitmask de compatibilidade do `.cfg` atual.
    pub fn compat(&self) -> CompatFlags {
        self.get(CONFIG_ITEM_COMPAT)
            .map(CompatFlags::parse)
            .unwrap_or_default()
    }

    /// Aplica o bitmask **preservando o resto** (read-modify-write): bits setados
    /// gravam/atualizam `$Compatibility`; tudo zerado **remove** a chave. Campos
    /// de info e outras chaves `$` nunca são tocados.
    pub fn apply_compat(&mut self, flags: &CompatFlags) {
        match flags.to_config_value() {
            Some(v) => self.set(CONFIG_ITEM_COMPAT, &v),
            None => self.remove(CONFIG_ITEM_COMPAT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_info::KEY_GENRE;

    #[test]
    fn parse_le_bitmask_decimal() {
        // 0x01 | 0x04 = 5 → Modes 1 e 3.
        let f = CompatFlags::parse("5");
        assert!(f.is_set(CompatMode::AccurateReads));
        assert!(f.is_set(CompatMode::UnhookSyscalls));
        assert!(!f.is_set(CompatMode::SynchronousMode));
    }

    #[test]
    fn parse_de_valor_invalido_ou_vazio_e_zero() {
        assert!(CompatFlags::parse("").is_empty());
        assert!(CompatFlags::parse("abc").is_empty());
    }

    #[test]
    fn to_config_value_zero_vira_none() {
        assert_eq!(CompatFlags::default().to_config_value(), None);
        let mut f = CompatFlags::default();
        f.set(CompatMode::SkipVideos, true);
        assert_eq!(f.to_config_value(), Some("8".to_string()));
    }

    #[test]
    fn set_preserva_outros_bits() {
        let mut f = CompatFlags::parse("5"); // Modes 1+3
        f.set(CompatMode::SynchronousMode, true); // +Mode 2 (0x02)
        assert_eq!(f.to_config_value(), Some("7".to_string()));
        f.set(CompatMode::AccurateReads, false); // -Mode 1 (0x01)
        assert_eq!(f.to_config_value(), Some("6".to_string()));
    }

    /// TRAVA: bits dos Modes 7/8 (sem UI) precisam sobreviver a um round-trip —
    /// a UI mexe só nos 6 expostos e não pode estragar o valor.
    #[test]
    fn bits_desconhecidos_7_8_sao_preservados() {
        let mut f = CompatFlags::parse("64"); // 0x40 = Mode 7
        assert!(!f.is_empty());
        f.set(CompatMode::DisableIgr, true); // +0x20
        // 0x40 | 0x20 = 96, com o bit 7 intacto.
        assert_eq!(f.to_config_value(), Some("96".to_string()));
    }

    #[test]
    fn compat_e_apply_compat_round_trip_no_cfg() {
        let mut cfg = GameCfg::parse("Genre=Action\n$Compatibility=5\n");
        let f = cfg.compat();
        assert!(f.is_set(CompatMode::AccurateReads));
        assert!(f.is_set(CompatMode::UnhookSyscalls));

        let mut f2 = f;
        f2.set(CompatMode::DisableIgr, true);
        cfg.apply_compat(&f2);
        assert_eq!(cfg.get(CONFIG_ITEM_COMPAT), Some("37")); // 5 | 0x20
        // Info intacta.
        assert_eq!(cfg.get(KEY_GENRE), Some("Action"));
    }

    /// TRAVA: aplicar compat não pode tocar nos campos de info nem em outras `$`.
    #[test]
    fn apply_compat_preserva_info_e_outras_chaves() {
        let mut cfg = GameCfg::parse("Title=GoW\n$VMC_0=Save\n$Compatibility=1\n");
        let mut f = cfg.compat();
        f.set(CompatMode::SkipVideos, true);
        cfg.apply_compat(&f);
        assert_eq!(cfg.get("Title"), Some("GoW"));
        assert_eq!(cfg.get("$VMC_0"), Some("Save"));
        assert_eq!(cfg.get(CONFIG_ITEM_COMPAT), Some("9")); // 1 | 8
    }

    #[test]
    fn apply_compat_zero_remove_a_chave() {
        let mut cfg = GameCfg::parse("$Compatibility=4\nGenre=Action\n");
        cfg.apply_compat(&CompatFlags::default());
        assert_eq!(cfg.get(CONFIG_ITEM_COMPAT), None);
        assert_eq!(cfg.get(KEY_GENRE), Some("Action"));
    }

    #[test]
    fn todos_os_modos_tem_bits_e_numeros_distintos() {
        // Sanidade: os 6 bits batem com COMPAT_MODE_1..6 e os números são 1..6.
        let bits: Vec<u8> = CompatMode::ALL.iter().map(|m| m.bit()).collect();
        assert_eq!(bits, vec![0x01, 0x02, 0x04, 0x08, 0x10, 0x20]);
        let nums: Vec<u8> = CompatMode::ALL.iter().map(|m| m.number()).collect();
        assert_eq!(nums, vec![1, 2, 3, 4, 5, 6]);
    }
}
