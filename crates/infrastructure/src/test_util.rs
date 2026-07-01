//! Utilitários compartilhados pelos testes da infraestrutura. Só existe sob
//! `cfg(test)`. Centraliza o padrão de caminho temporário único (PID + contador
//! atômico) que vivia copiado em cada módulo de teste.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Caminho único em `std::env::temp_dir()` para um teste, **sem criar** nada no
/// disco (cada teste decide se cria dir ou grava arquivo). Único mesmo com os
/// testes rodando em paralelo: combina PID + um contador atômico do processo de
/// teste. `prefix` distingue o módulo de origem (ex.: `"scan"`, `"art"`).
pub fn unique_path(prefix: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("oplhost-{prefix}-{}-{n}", std::process::id()))
}
