//! Binário principal do oplhost. Liga a janela Slint aos callbacks.
//!
//! Scaffold da Fase 1: a janela abre e responde aos botões com placeholders. A
//! fiação real com `StorageBackend`/`SmbBackend` entra à medida que a Fase 1
//! avança, sempre via mensagens/callbacks (a UI não fala com a infra direto).

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    ui.set_status_text("Parado".into());
    ui.set_ip_text(local_ip_placeholder().into());
    ui.set_share_path("(nenhum diretório selecionado)".into());

    ui.on_choose_dir_clicked(|| {
        // TODO(fase-1): abrir seletor de diretório e atualizar o share-path.
        println!("escolher diretório (placeholder)");
    });
    ui.on_start_clicked(|| {
        // TODO(fase-1): apply_config + start via StorageBackend (janela Polkit).
        println!("iniciar servidor (placeholder)");
    });
    ui.on_stop_clicked(|| {
        // TODO(fase-1): stop + rollback via StorageBackend.
        println!("parar servidor (placeholder)");
    });

    ui.run()
}

/// Placeholder até a infraestrutura de rede ser fiada. A descoberta real do IP
/// local exibido para o OPL entra na Fase 1.
fn local_ip_placeholder() -> &'static str {
    "—"
}
