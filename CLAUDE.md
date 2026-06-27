# CLAUDE.md — `oplhost` (nome provisório, trocável)

> Aplicação gráfica em Rust para Linux que automatiza a criação e o gerenciamento de um **servidor Samba (SMB) dedicado ao Open PS2 Loader (OPL)**. O propósito central não é gerenciar jogos (já existem ferramentas para isso) — é **eliminar a dor de configurar manualmente um share SMBv1 funcional, com firewall e privilégios, no Linux**.

Este é o documento de **regras** do projeto e a fonte de verdade de estrutura e padrões. Leia por inteiro antes de escrever código. O *histórico* de decisões (o porquê de cada escolha, alternativas, andamento) fica em `plans/` — não aqui.

---

## 0. Princípio que define o projeto: o OPL só fala SMBv1

**É o coração técnico do projeto.** O Open PS2 Loader só conecta via **SMBv1 (CIFS / NT1)**, protocolo obsoleto e **desabilitado por padrão** no Samba moderno (4.11+). Um share guest comum não funciona — o OPL não conecta.

Consequências para a implementação:

- O `opl_share.conf` gerado **deve forçar protocolo legado**. Esqueleto de referência, **validado na Fase 0 com Samba 4.23.6** (NT1 guest lê e escreve — ver `plans/fase-0-spike.md`). Valores podem variar por versão do Samba; reconfirmar com `testparm` ao mudar de ambiente:

  ```ini
  [global]
     client min protocol = NT1
     server min protocol = NT1
     ntlm auth = yes
     lanman auth = yes
     usershare allow guests = yes

  [PS2SMB]
     comment = OPL Share
     path = /caminho/escolhido/pelo/usuario
     guest ok = yes
     read only = no
     force user = <usuario_dono_da_pasta>
  ```

- O sistema loga avisos do tipo "Weak crypto is allowed" e "lanman auth deprecated". **Isso é esperado.** A UI **deve comunicar** ao usuário que o app reabilita um protocolo legado por exigência do OPL — transparência é requisito de UX.
- Porta padrão **445** (configurável como avançado; o OPL aceita portas alternativas).
- **Isolamento (regra inegociável):** nunca editar o `/etc/samba/smb.conf` global diretamente. Injetar apenas a linha `include = /etc/samba/opl_share.conf` e gerenciar todo o conteúdo legado nesse arquivo isolado. Rollback = remover o arquivo + a linha de include.

---

## 1. Escopo e plataforma

- **Plataforma:** Linux exclusivamente. Não criar abstrações cross-platform especulativas.
- **Servidor gráfico:** Wayland como alvo primário, X11 por retrocompatibilidade.
- **Ambientes:** GNOME, Cinnamon, MATE, XFCE.
- **Dependências de sistema:** `samba` e `polkit` (obrigatórias); `ufw` ou `iptables`/`iptables-persistent` (tratados dinamicamente).

---

## 2. Stack técnica

- **Linguagem:** Rust.
- **GUI:** **Slint** (DSL `.slint` + lógica em Rust). A camada `ui` deve permanecer **desacoplada o suficiente para troca por egui sem tocar em `core`** — manter essa propriedade é uma regra, não um detalhe.
- **System Tray:** feature opcional de fase tardia, atrás de feature flag. **A janela principal e toda a lógica de servidor funcionam 100% sem tray.** Nunca amarrar o ciclo de vida do app ao tray. Em Wayland o caminho é SNI/StatusNotifierItem sobre D-Bus (crate de referência: `ksni`).
- **D-Bus / Polkit:** crate `zbus`.
- **Serialização:** `serde` + `serde_json` (para `opl_meta.json`).
- **i18n:** `fluent` (fluent-rs), pt-BR e en-US embutidos no início.
- **FTP (opcional, fase tardia):** `suppaftp`, só se o cenário USB/console for incorporado.

---

## 3. Arquitetura — Clean Architecture (Ports & Adapters) + SOLID

Três camadas. Inversão de dependência via **Traits** para permitir mocking e testes. Implementada como **workspace Cargo com 3 crates** (`core`, `infrastructure`, `ui`), de modo que a inversão de dependência seja imposta pelo compilador.

### `core` (regras de negócio, agnóstico a I/O / rede / UI)
- Estruturação de pastas do OPL.
- Parser e validador de metadados.
- Cálculo de contagem, tamanho em disco, categorização.
- **Define os Traits** (ports) que a infraestrutura implementa.

### `infrastructure` (adapters — implementações reais)
- `StorageBackend` (Trait genérico, **não casado com SMB**): `start`, `stop`, `status`, `apply_config`, `rollback`. Primeira implementação: `SmbBackend`. Há um segundo backend (UDPBD) no horizonte (§7.1); o Trait deve acomodá-lo sem refatoração dolorosa. **Nunca assumir no código que "o alvo é sempre uma pasta SMB com smb.conf".**
- `SmbBackend`: gera `opl_share.conf`, injeta/remove o include, controla `smbd`.
- `FirewallManager`: detecta `ufw` (ativo/inativo) → cria regra (porta depende do backend: TCP 445 para SMB); fallback `iptables`; persistência via `iptables-persistent`.
- `PrivilegeEscalator`: dispara o prompt nativo do Polkit para operações root.
- `ArtProvider` (fase 2): lê/baixa art das fontes externas (§7).
- `MetaStore`: persistência do `opl_meta.json`.

### `ui` (apresentação — Slint)
- Consome o `core`. Estado da interface **isolado da lógica de negócio**.
- Comunicação via mensagens/callbacks, **nunca** acessando `infrastructure` diretamente.

### Definição dos Traits
Defini-los a partir do que o problema real exige (a Fase 0 já revelou as operações root necessárias). Abstrair antes de entender o problema gera as piores abstrações.

### Regras de código
- Arquivos curtos e especializados (alta granularidade).
- Inversão de dependência em todos os pontos de I/O.
- Cada componente com responsabilidade única, testável isoladamente.
- Preferir loops explícitos e código legível a combinadores densos quando a clareza estiver em jogo.

---

## 4. Estrutura de pastas do OPL

O OPL **descobre os jogos pela estrutura de pastas diretamente** — ele NÃO lê nenhum arquivo da nossa aplicação. A estrutura é injetada na raiz do diretório-alvo escolhido pelo usuário (HDD externo, pendrive ou pasta local):

- `CD/` e `DVD/` — ISOs (CD = jogos ≤ 700MB; DVD = o resto).
- `ART/` — capas e artwork (mesmo dispositivo do jogo, nomeado por Game ID).
- `THM/` — temas. `VMC/` — virtual memory cards.
- `CFG/`, `CHT/`, `LNG/`, `APPS/`, `POPS/` — configs, cheats, idiomas, homebrew, PS1.

Manipulação de ISOs: **não copiar/mover arquivos grandes entre partições.** Organizar dentro da estrutura criada no próprio diretório-alvo.

---

## 5. Privilégios (Polkit) e Firewall

- App roda em **user-space**. Operações root (reiniciar `smbd`, mexer no firewall, criar `opl_share.conf` em `/etc/samba/`) disparam o **Polkit** para o prompt nativo de senha.
- **Agrupar operações root numa única "janela de privilégio"** sempre que possível, para não pedir senha repetidamente. (Validado na Fase 0: script único via um `pkexec`.)
- Rotina de firewall na janela root: verificar `ufw` → se ativo e sem regra SMB, criar; se ausente/inativo, fallback `iptables`; garantir persistência.

---

## 6. Persistência: `opl_meta.json`

- Arquivo na raiz do diretório selecionado (portabilidade se o usuário mover o disco).
- **É metadado DA NOSSA aplicação, não do OPL.** Cache de conveniência: nomes, categorias, ano, tamanho, contagem.
- **Requisito crítico:** o app funciona **mesmo se o JSON for apagado**, relendo `CD/DVD` para reconstruir o estado. Nunca tratar o JSON como fonte de verdade única.

---

## 7. Reutilização e fontes externas

**Não fazer web scraping de capas.** As fontes já existem e são consumíveis:

- **Art databases prontos:** set do Kira (~61k imagens) e os backups mensais do OPL Manager (danielb) no archive.org. O app baixa/lê dessas fontes.
- **Referência de domínio:** o **PyOPLM** (open source, CLI Python) resolve a lógica de gerenciar diretório OPL (nomear ART por Game ID, identificar jogos, estruturar pastas). **Estudar a lógica e reescrever em Rust** — não importar código Python.
- **FTP remoto do console (opcional, fase tardia):** só faz sentido no cenário USB/HDD-interno (arquivos no PS2). Implementar com `suppaftp` apenas se cobrir esse cenário — não é necessário para o produto principal (servidor SMB), onde os arquivos já estão locais no Linux.

### 7.1 Backend UDPBD — evolução planejada (NÃO implementar na V1)

O ecossistema OPL caminha para tratar rede, USB e iLink como "block devices" intercambiáveis via **BDM (Block Device Manager)**. O backend de rede dessa família é o **UDPBD** (do rickgaiser): mais rápido que SMB, usa menos RAM/CPU do PS2 e sem o problema de segurança do SMBv1. Hoje é beta e exige um build do OPL com BDM no console.

**Diferença de modelo:** UDPBD não compartilha um filesystem — serve um **block device bruto** (FAT32/ExFAT ou imagem) via UDP (porta 48573 / `0xbdbd`). Sem Samba, sem `smb.conf`, sem autenticação. Mas a **estrutura de pastas do OPL é idêntica** → a camada `core` é 100% reaproveitável; muda só o adapter.

**Diretrizes de design (aplicar desde já, sem implementar):**
- O Trait `StorageBackend` já deve comportar um futuro `UdpbdBackend` sem refatoração dolorosa. Não casar o código com pressupostos de SMB.
- **Implementar SMB primeiro. Só refatorar a abstração ao adicionar UDPBD**, com os dois casos concretos na mão.
- **Não reimplementar o protocolo UDPBD em Rust.** Supervisionar um servidor existente (`udpbd-server` do rickgaiser, ou `neutrino`/`udpfs`).
- Posicionamento de produto: múltiplos backends transformam o app de "configurador de Samba" em **"gerenciador unificado de servidores OPL para Linux"** (SMB hoje, UDPBD amanhã).

---

## 8. Tratamento de erros e UX

- Captura exaustiva de falhas externas: daemon Samba, **porta 445 ocupada**, falha na delegação do Polkit, regras de firewall.
- Notificações descritivas na GUI orientando a resolução (ex: "porta 445 em uso por outro serviço") — **sem crash**.
- Exibir IP local e instruções de conexão para o OPL na tela principal.
- Identificação imediata do status do servidor (rodando/parado) e listagem do catálogo.

---

## 9. Empacotamento e distribuição

- **`.deb` na V1.** O app precisa de D-Bus do sistema, Polkit, escrita em `/etc/samba/` e firewall — o confinamento de Snap/Flatpak conflita com isso. Snap/Flatpak ficam como possibilidade futura só se houver demanda real.
- Scripts `postinst` validam dependências do sistema (`samba`, `polkit`).

---

## 10. Qualidade, testes e Git

- **Versionamento:** Git. Integração na `main` só via Pull Request.
- **Testes:** cobertura unitária mínima de **70% focada no `core`** (parsers, validadores de metadados, estruturação de pastas). Construir testes iterativamente junto ao código.
- Os Traits permitem mockar `infrastructure` nos testes do `core`.
- **Cada fase de desenvolvimento tem um plano em `plans/`** seguindo o `PLANS_TEMPLATE.md`, com as decisões registradas e atualizado a cada mudança (commitado).

---

## 11. Roadmap por fases

### Fase 0 — Spike de validação (gate obrigatório) — ver `plans/fase-0-spike.md`
Binário descartável que prova a conexão SMBv1. **Validado localmente** (Samba 4.23.6, NT1 guest lê/escreve); pendente apenas a confirmação com PS2/OPL real. Só então o spike é removido e a fase, concluída.

### Fase 1 — Núcleo funcional — ver `plans/fase-1-nucleo.md`
- Workspace `core`/`infrastructure`/`ui` com Traits definidos a partir da Fase 0.
- GUI Slint: status do servidor, seleção de diretório, start/stop, IP e instruções.
- Geração/rollback do share isolado. Firewall. Polkit.
- Injeção da estrutura de pastas OPL. `opl_meta.json`. Testes do `core` ≥ 70%. Empacotamento `.deb`.

### Fase 2 — Biblioteca e metadados
- Listagem rica do catálogo (CD/DVD). Consumo de art databases externos (Kira / danielb-archive.org). Categorização, contagem, tamanho em disco. Autenticação usuário/senha opcional no share.

### Fase 3 — Refinamento e opcionais
- i18n com arquivos externos para a comunidade. System tray (atrás de flag, SNI/`ksni`). **`UdpbdBackend`** via supervisão de servidor existente (refatorar `StorageBackend` aqui, com SMB como segundo caso concreto). FTP remoto (opcional, `suppaftp`). Integração com listas de compatibilidade por jogo.

---

## 12. Regras de trabalho para o Claude Code

- Trabalhar **fase a fase**. Não pular a Fase 0.
- Arquivos curtos, responsabilidade única, SOLID. Evitar abstrações especulativas.
- Escrever testes junto ao código do `core`, não depois.
- Quando um valor de config Samba/firewall for incerto, **marcar como "validar no ambiente"** em vez de assumir que um exemplo de fórum é verdade absoluta — versões de Samba divergem.
- Manter a camada `ui` desacoplada o suficiente para a troca Slint→egui ser viável.
- Comunicar ao usuário (na UI) qualquer operação sensível: reabilitação de SMBv1, alteração de firewall, escalonamento de privilégio.
- Manter o plano da fase em `plans/` atualizado e commitar as mudanças.
