//! Utilitários de rede para a UI: IP local a exibir nas instruções do OPL e
//! checagem de porta ocupada (§8). Só `std`, sem dependências externas.

use std::net::{TcpStream, UdpSocket};
use std::time::Duration;

/// Descobre o IP local provável da máquina na LAN — o que o usuário digita no
/// OPL. Truque sem tráfego real: "conecta" um socket UDP a um destino externo e
/// lê o endereço local que o SO escolheu para a rota. Não envia pacote algum.
/// Retorna `None` se não houver rota (ex.: máquina offline).
pub fn local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // 8.8.8.8 é só um alvo de rota; `connect` em UDP não gera tráfego.
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

/// Best-effort: `true` se algo já aceita conexão TCP na porta em `127.0.0.1`.
/// Usado antes do apply para detectar a porta 445 tomada por outro serviço e
/// dar a mensagem do §8 em vez de deixar o `smbd` falhar silenciosamente.
pub fn tcp_port_listening(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    match addr.parse() {
        Ok(sock) => TcpStream::connect_timeout(&sock, Duration::from_millis(300)).is_ok(),
        Err(_) => false,
    }
}
