# CLAUDE.md — `oplhost` (nome provisório, trocável)

> Aplicação gráfica em Rust para Linux que automatiza a criação e o gerenciamento de um **servidor Samba (SMB) dedicado ao Open PS2 Loader (OPL)**. O propósito central não é gerenciar jogos (já existem ferramentas para isso) — é **eliminar a dor de configurar manualmente um share SMBv1 funcional, com firewall e privilégios, no Linux**.

Este documento é a especificação-guia do projeto. Leia por inteiro antes de escrever código. As decisões aqui foram tomadas após pesquisa do ecossistema real do OPL e dos riscos técnicos da stack; não as reverta sem motivo explícito.

---

## 0. Princípio que define o projeto inteiro: o OPL só fala SMBv1

**Isto não é um detalhe. É o coração técnico do projeto.**

O Open PS2 Loader só consegue conectar via **SMBv1 (CIFS / NT1)**. O protocolo está obsoleto e vem **desabilitado por padrão** no Samba moderno (4.11+), marcado como inseguro. Um share guest comum **não funciona** — o OPL não conecta.

Consequências diretas para a implementação:

- O `opl_share.conf` gerado **deve forçar protocolo legado**. Esqueleto de referência (extraído de configs reais que funcionam em fóruns — **deve ser validado no spike da Fase 0**, pois os valores exatos variam por versão do Samba):

  ```ini
  # Em [global] (via include):
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

- O sistema vai logar avisos do tipo "Weak crypto is allowed". **Isso é esperado.** A UI deve **comunicar ao usuário** que o app reabilita um protocolo legado por exigência do OPL — não esconder isso. Transparência é requisito de UX, não opcional.
- A porta padrão é **445**, mas o OPL aceita portas alternativas (há setups usando 1024). Manter 445 como padrão, porta configurável como avançado.

**Decisão de isolamento (mantida da spec original — é a melhor decisão do projeto):** nunca editar o `/etc/samba/smb.conf` global diretamente. Injetar apenas a linha `include = /etc/samba/opl_share.conf` e gerenciar todo o conteúdo perigoso nesse arquivo isolado. Rollback = remover o arquivo + a linha de include.

---

## 1. Escopo e plataforma

- **Plataforma:** Linux **exclusivamente**. Windows foi descartado (o nicho Windows já é coberto por OPL Manager e OPL Server; a lacuna real é Linux). Não criar abstrações "cross-platform" especulativas.
- **Servidor gráfico:** Wayland como alvo primário, X11 por retrocompatibilidade.
- **Ambientes:** GNOME, Cinnamon, MATE, XFCE.
- **Dependências de sistema:** `samba` (obrigatório), `polkit` (obrigatório), `ufw` ou `iptables`/`iptables-persistent` (tratados dinamicamente).

---

## 2. Stack técnica (decisões fechadas)

- **Linguagem:** Rust.
- **GUI:** **Slint** (DSL declarativa `.slint` + lógica em Rust). Escolhido por: separação UI/lógica que casa com Clean Architecture e é fácil de gerar/iterar via Claude Code, estética elegante e consistente entre DEs, live preview, API 1.x estável, licença GPLv3 compatível com open source.
  - **Plano B explícito:** se o Slint causar atrito de build/integração que trave o progresso, migrar para **egui** (mais simples e rápido, menos elegante). A arquitetura DEVE manter a `ui` desacoplada o suficiente para permitir essa troca sem tocar em `core`.
- **System Tray:** **feature opcional de fase tardia, atrás de feature flag.** NÃO é requisito core. Razões: (a) em Wayland o protocolo XEmbed clássico não funciona — o caminho é SNI/StatusNotifierItem sobre D-Bus (crate de referência: `ksni`, confirmar versão atual); (b) o GNOME removeu a área de tray e exige extensão de terceiros (AppIndicator). **A janela principal e toda a lógica de servidor devem funcionar 100% sem tray.** Nunca amarrar o ciclo de vida do app ao tray.
- **D-Bus / Polkit:** crate `zbus` (referência atual para D-Bus em Rust).
- **Serialização:** `serde` + `serde_json` (para `opl_meta.json`).
- **i18n:** `fluent` (fluent-rs). Começar com pt-BR e en-US embutidos.
- **FTP (opcional, fase tardia):** `suppaftp` — apenas se o cenário USB/console for incorporado depois.

---

## 3. Arquitetura — Clean Architecture (Ports & Adapters) + SOLID

Três camadas. Inversão de dependência via **Traits** para permitir mocking e testes.

### `core` (regras de negócio, agnóstico a I/O / rede / UI)
- Estruturação de pastas do OPL.
- Parser e validador de metadados.
- Cálculo de contagem, tamanho em disco, categorização.
- **Define os Traits** (ports) que a infraestrutura implementa.

### `infrastructure` (adapters — implementações reais)
- `StorageBackend` (Trait genérico, **não casado com SMB**): operações como `start`, `stop`, `status`, `apply_config`. Primeira e única implementação na V1: `SmbBackend`. Ver seção 7.1 — há um segundo backend (UDPBD) no horizonte, e o Trait deve ser desenhado para acomodá-lo sem refatoração dolorosa. **Não assumir em lugar nenhum do código que "o alvo é sempre uma pasta SMB com smb.conf".**
- `SmbBackend` (impl de `StorageBackend`): gera `opl_share.conf`, injeta/remove o include, controla `smbd`.
- `FirewallManager`: detecta `ufw` (ativo/inativo) → cria regras (porta depende do backend: TCP 445 para SMB); fallback `iptables`; garante persistência via `iptables-persistent`.
- `PrivilegeEscalator`: dispara o prompt nativo do Polkit para operações root.
- `ArtProvider` (fase 2): baixa/lê art das fontes externas (ver seção 7).
- `MetaStore`: persistência do `opl_meta.json`.

### `ui` (apresentação — Slint)
- Consome o `core`. Gerenciamento de estado da interface **isolado da lógica de negócio**.
- Comunicação com a lógica via mensagens/callbacks, nunca acessando `infrastructure` diretamente.

### Definição dos Traits
**Não desenhar os Traits a priori.** Defini-los a partir do que o spike da Fase 0 revelar que a infraestrutura realmente precisa fazer. Abstrair antes de entender o problema gera as piores abstrações.

### Regras de código
- Arquivos curtos e especializados (alta granularidade).
- Inversão de dependência em todos os pontos de I/O.
- Sem "vibe coding": cada componente com responsabilidade única, testável isoladamente.

---

## 4. Estrutura de pastas do OPL

O OPL **descobre os jogos pela estrutura de pastas diretamente** — ele NÃO lê nenhum arquivo da nossa aplicação. A estrutura é injetada na raiz do diretório alvo escolhido pelo usuário (HDD externo, pendrive ou pasta local):

- `CD/` e `DVD/` — ISOs (CD = jogos ≤ 700MB; DVD = o resto).
- `ART/` — capas e artwork (mesmo dispositivo do jogo, nomeado por Game ID).
- `THM/` — temas.
- `VMC/` — virtual memory cards.
- `CFG/`, `CHT/`, `LNG/`, `APPS/`, `POPS/` — configs, cheats, idiomas, homebrew, PS1.

Manipulação de ISOs: **não copiar/mover arquivos grandes entre partições.** Organizar dentro da estrutura criada no próprio diretório alvo.

---

## 5. Privilégios (Polkit) e Firewall

- App roda em **user-space**. Operações root (reiniciar `smbd`, mexer no firewall, criar `opl_share.conf` em `/etc/samba/`) disparam o **Polkit** para o prompt nativo de senha do sistema.
- Agrupar operações root numa única "janela de privilégio" quando possível, para não pedir senha repetidamente.
- Rotina de firewall durante a janela root: verificar `ufw` → se ativo e sem regras SMB, criar; se ausente/inativo, fallback `iptables`; garantir persistência.

---

## 6. Persistência: `opl_meta.json`

- Arquivo na raiz do diretório selecionado (portabilidade se o usuário mover o disco).
- **É metadado DA NOSSA aplicação, não do OPL.** Cache de conveniência: nomes, categorias, ano, tamanho, contagem.
- **Requisito crítico:** o app deve funcionar **mesmo se o JSON for apagado**, relendo as pastas `CD/DVD` para reconstruir o estado. Nunca tratar o JSON como fonte de verdade única.

---

## 7. Reutilização e fontes externas (substitui boa parte da "seção 10")

**Não fazer web scraping de capas.** As fontes já existem e são consumíveis:

- **Art databases prontos:** set do Kira (~61k imagens) e os backups mensais do OPL Manager (danielb) hospedados no archive.org. O app baixa/lê dessas fontes — não raspa sites (que quebram).
- **Referência de domínio:** o **PyOPLM** (open source, CLI Python) resolve a lógica de gerenciar diretório OPL (nomear ART por Game ID, identificar jogos, estruturar pastas). **Estudar a lógica e reescrever em Rust** — não importar código Python.
- **FTP remoto do console (opcional, fase tardia):** só faz sentido no cenário USB/HDD-interno (arquivos no PS2, não no servidor). Precedente: o FTP sync do OPL Manager. Implementar com `suppaftp` apenas se decidir cobrir esse cenário — **não é necessário para o produto principal (servidor SMB)**, onde os arquivos já estão locais no Linux.

**Descartado definitivamente:** instalador direto no PS2 via rede (território de exploit/homebrew de baixo nível, fora de escopo).

### 7.1 Backend UDPBD — evolução planejada (NÃO implementar na V1)

O ecossistema OPL caminha para tratar rede, USB e iLink como "block devices" intercambiáveis via **BDM (Block Device Manager)**. O backend de rede dessa família é o **UDPBD** (UDP Block Device, do rickgaiser): mais rápido que SMB, usa menos RAM/CPU do PS2 e **não tem o problema de segurança do SMBv1**. Hoje ainda é **beta** e exige um build do OPL com BDM no lado do console.

**Diferença de modelo (importante):** UDPBD não compartilha um filesystem — serve um **block device bruto formatado em FAT32/ExFAT** (ou uma imagem) via UDP (porta 48573 / `0xbdbd`). Sem Samba, sem `smb.conf`, **sem autenticação**. Mas a **estrutura de pastas do OPL é idêntica** à do SMB → a camada `core` é 100% reaproveitável; muda só o adapter.

**Diretrizes de design (aplicar desde já, sem implementar):**
- O Trait `StorageBackend` (seção 3) já deve comportar um futuro `UdpbdBackend` sem refatoração dolorosa. Não casar o código com pressupostos de SMB.
- **Implementar SMB primeiro (Fases 1–2). Só refatorar a abstração quando for adicionar UDPBD**, com os dois casos concretos na mão — abstração boa nasce de dois exemplos reais, não de um imaginado.
- **Não reimplementar o protocolo UDPBD em Rust.** Supervisionar um servidor existente (`udpbd-server` do rickgaiser/ps2max, ou o `neutrino`/`udpfs` — este último serve block device, imagem ou diretório, com descompressão transparente de `.zso/.cso/.chd` → `.iso`). Reimplementar em Rust é projeto à parte, decisão para muito depois.
- Posicionamento de produto: assumir múltiplos backends desde o conceito transforma o app de "configurador de Samba" em **"gerenciador unificado de servidores OPL para Linux"** (SMB estável hoje, UDPBD rápido amanhã) — diferencial mais forte e mais duradouro, já que o SMB é justamente o que o ecossistema tenta aposentar.

---

## 8. Tratamento de erros e UX

- Captura exaustiva de falhas externas: daemon Samba, **porta 445 ocupada**, falha na delegação do Polkit, regras de firewall.
- Notificações descritivas na GUI orientando a resolução (ex: "porta 445 em uso por outro serviço") — **sem crash**.
- Exibir IP local e instruções de conexão para o OPL na tela principal.
- Identificação imediata do status do servidor (rodando/parado) e listagem do catálogo.

---

## 9. Empacotamento e distribuição

- **Apenas `.deb` na V1.** Snap/Flatpak foram descartados para a V1: o confinamento (sandbox) briga diretamente com D-Bus do sistema, Polkit, escrita em `/etc/samba/` e firewall — gera dias de luta contra interfaces/plugs. `.deb` é o caminho certo para um app que precisa de privilégios de sistema.
- Scripts `postinst` validam dependências do sistema (`samba`, `polkit`).
- Snap/Flatpak ficam como possibilidade futura **só** se houver demanda real.

---

## 10. Qualidade, testes e Git

- **Versionamento:** Git. Merge na `main` bloqueado; integração só via Pull Request.
- **Testes:** cobertura unitária mínima de **70% focada no `core`** (parsers, validadores de metadados, estruturação de pastas). Construir testes iterativamente junto ao código.
- Os Traits permitem mockar `infrastructure` nos testes do `core`.

---

## 11. Roadmap por fases

### Fase 0 — SPIKE DE VALIDAÇÃO (gate obrigatório antes de qualquer arquitetura)
Binário Rust **descartável, sem arquitetura nenhuma**, que apenas:
1. Gera o `opl_share.conf` com SMBv1.
2. Injeta o `include` no `smb.conf`.
3. Reinicia o `smbd` via Polkit.
4. Abre a porta 445.

**Critério de sucesso:** conectar de um cliente SMBv1 real (idealmente um PS2 com OPL; o usuário tinha um setup funcional há ~2 anos e vai reverificar). **Se esta parte não funcionar limpa, nada mais importa.** Validar isto ANTES de construir a Clean Architecture em volta. Os aprendizados deste spike definem os Traits da Fase 1.

### Fase 1 — Núcleo funcional
- Arquitetura `core`/`infrastructure`/`ui` com Traits definidos a partir da Fase 0.
- GUI Slint: status do servidor, seleção de diretório, start/stop, IP e instruções.
- Geração/rollback do share isolado. Firewall. Polkit.
- Injeção da estrutura de pastas OPL. `opl_meta.json`.
- Testes do `core` ≥ 70%.
- Empacotamento `.deb`.

### Fase 2 — Biblioteca e metadados
- Listagem rica do catálogo (CD/DVD).
- Consumo de art databases externos (Kira / danielb-archive.org).
- Categorização, contagem, tamanho em disco.
- Autenticação usuário/senha opcional no share.

### Fase 3 — Refinamento e opcionais
- i18n com arquivos externos para a comunidade.
- System tray (atrás de flag, SNI/`ksni`).
- **(Backend alternativo) `UdpbdBackend`** via supervisão de servidor existente (ver 7.1). Refatorar a abstração `StorageBackend` aqui, com SMB já implementado como segundo caso concreto.
- (Opcional) FTP remoto para cenário USB/console via `suppaftp`.
- (Opcional) Integração com listas de compatibilidade por jogo.

---

## 12. Regras de trabalho para o Claude Code

- Trabalhar **fase a fase**. Não pular a Fase 0.
- Arquivos curtos, responsabilidade única, SOLID. Evitar abstrações especulativas.
- Escrever testes junto ao código do `core`, não depois.
- Quando um valor de config Samba/firewall for incerto, **marcar como "validar no ambiente"** em vez de assumir que um exemplo de fórum é verdade absoluta — versões de Samba divergem.
- Manter a camada `ui` desacoplada o suficiente para a troca Slint→egui ser viável.
- Comunicar ao usuário (na UI) qualquer operação sensível: reabilitação de SMBv1, alteração de firewall, escalonamento de privilégio.
- Preferir loops explícitos e código legível a combinadores densos quando a clareza estiver em jogo (preferência do autor neste estágio de Rust).