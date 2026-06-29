# oplhost

Servidor SMB dedicado ao Open PS2 Loader (OPL), para Linux.

O `oplhost` é uma aplicação gráfica que automatiza a criação e o gerenciamento de
um servidor Samba (SMB) compatível com o Open PS2 Loader. O objetivo não é
gerenciar jogos — já existem ferramentas para isso — e sim **eliminar a dor de
configurar manualmente um compartilhamento SMB que o OPL aceite**, com firewall e
privilégios resolvidos, sem editar arquivos de sistema à mão.

## Por que é necessário

O Open PS2 Loader só conecta via **SMBv1 (CIFS / NT1)**, um protocolo legado que
vem **desabilitado por padrão** no Samba moderno (4.11+). Por isso um
compartilhamento comum não funciona com o OPL: o console simplesmente não conecta.

O `oplhost` reabilita o SMBv1 de forma controlada — num arquivo de configuração
**isolado e reversível**, sem nunca alterar o conteúdo do `smb.conf` global —,
abre a porta no firewall e executa as operações privilegiadas através do Polkit,
pedindo a senha uma única vez.

## Funcionalidades

- **Compartilhamento pronto para o OPL.** Gera um share SMBv1 (NT1) isolado a
  partir do diretório que você escolher (HD externo, pendrive ou pasta local).
- **Ativar/desativar com um clique.** Um único controle aplica ou remove a
  configuração. A remoção é um rollback completo: o sistema volta ao estado
  anterior, sem vestígios.
- **Estrutura de pastas do OPL.** Cria a estrutura que o OPL espera (`CD`, `DVD`,
  `ART`, `THM`, `VMC`, `CFG`, `CHT`, `LNG`, `APPS`, `POPS`) no diretório-alvo, se
  ainda não existir.
- **Catálogo do diretório.** Lista os jogos de `CD/` e `DVD/` com título, Game ID
  (lido do `SYSTEM.CNF` da ISO), mídia e tamanho, além da contagem e do tamanho
  total em disco.
- **Download de capas.** Baixa a arte de cada jogo por Game ID e grava em `ART/`
  com a nomenclatura do OPL, a partir de bancos de capas públicos.
- **Acesso livre ou autenticado.** Por padrão o share é guest (acesso livre); é
  possível exigir usuário e senha.
- **Firewall automático.** Abre a porta TCP 445 via `ufw`, com fallback para
  `iptables`.
- **Instruções de conexão.** Mostra o IP local da máquina e os dados para
  configurar o SMB no OPL.

## Compatibilidade testada

- **Open PS2 Loader:** versão `v1.2.0-beta-2012-b84c2b` (conexão SMB via Ethernet
  validada — guest e autenticado).
- **Samba:** 4.23.x.
- **Ambientes gráficos:** GNOME, Cinnamon, MATE e XFCE, em Wayland (alvo
  primário) ou X11.
- **Plataforma:** Linux exclusivamente.

## Requisitos de sistema

Dependências de runtime (o pacote `.deb` as declara e o instalador as resolve):

- `samba` — o daemon `smbd` que serve o compartilhamento.
- `polkit` (`pkexec` / `policykit-1`) — para as operações privilegiadas.
- `zenity` ou `kdialog` — para o seletor nativo de pasta.
- `ufw` ou `iptables` — para a regra de firewall (tratados dinamicamente).

## Instalação

### A partir do pacote `.deb` (recomendado)

Os pacotes são publicados em
[Releases](https://github.com/maicomferre/OPLHost/releases). Baixe o `.deb` da
versão desejada e instale com o `apt`, que resolve as dependências
automaticamente:

```bash
sudo apt install ./oplhost_0.1.0-1_amd64.deb
```

Em seguida, abra o `oplhost` pelo menu de aplicativos ou pelo terminal:

```bash
oplhost
```

### Gerando o `.deb` a partir do código

É preciso o toolchain Rust ([rustup](https://rustup.rs)), o
[`cargo-deb`](https://github.com/kornelski/cargo-deb) e as bibliotecas de
desenvolvimento usadas pela interface (Slint):

```bash
# Dependências de build (Debian/Ubuntu)
sudo apt install libfontconfig1-dev libfreetype6-dev libxcb1-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev

# Ferramenta de empacotamento
cargo install cargo-deb

# Gera o pacote em target/debian/
cargo deb -p oplhost
```

### Executando direto do código (desenvolvimento)

```bash
cargo run -p oplhost
```

## Uso

1. Em **Diretório-alvo do OPL**, escolha a pasta que será compartilhada (a que
   contém — ou conterá — `CD/`, `DVD/`, `ART/` etc.).
2. Clique em **Ativar servidor** e informe sua senha no prompt do sistema
   (Polkit). O `oplhost` cria o compartilhamento, recarrega o Samba e abre a
   porta.
3. No OPL, configure a conexão SMB com o **IP local** mostrado na tela, porta
   `445` e o share `PS2SMB`. Para acesso livre, use guest; se você ativou
   autenticação, informe o usuário e a senha definidos.
4. Atualize a lista de jogos no OPL.

Para reverter tudo, clique em **Desativar e reverter**.

## Transparência e segurança

O `oplhost` comunica na interface toda operação sensível, porque algumas são
inerentes ao funcionamento do OPL:

- **SMBv1 (protocolo legado).** É reabilitado por exigência do OPL, apenas no
  compartilhamento isolado do app. O Samba pode registrar avisos sobre criptografia
  fraca — isso é esperado.
- **Firewall.** A porta TCP 445 é aberta para permitir a conexão do console.
- **Privilégio.** As operações de root (configurar o Samba, mexer no firewall)
  são feitas via Polkit, agrupadas numa única solicitação de senha.
- **Isolamento.** O `smb.conf` global nunca é editado diretamente; o app injeta
  apenas uma linha de `include` apontando para um arquivo próprio. Desativar
  remove esse arquivo e a linha, restaurando o estado original.

## Licença

MIT. Veja `crates/ui/Cargo.toml` para autoria e metadados do pacote.
