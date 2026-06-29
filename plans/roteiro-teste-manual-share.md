# Roteiro de teste manual — fechamento da Fase 2

> Validação fim-a-fim do que os testes automatizados não alcançam: conexão real
> de um cliente SMB e do **OPL no PS2**, reconhecimento de capas, e o
> comportamento do daemon no ambiente. Este roteiro é o **gate de fechamento da
> Fase 2** — ao final, marcar os critérios pendentes em
> `plans/fase-2-biblioteca.md` e só então abrir o PR `fase-2-biblioteca → main`
> (CLAUDE.md §10: main só via PR).
>
> **Modelo "aplicar/remover config" (decisão 2026-06-28):** o app **não** dá
> start/stop no `smbd` global — ele grava o share isolado, injeta o `include` e
> dá `systemctl reload smbd`. Recarregar **não derruba** conexões nem outros usos
> do Samba na máquina. Pressuposto: o `smbd` já é gerenciado pelo sistema (o app
> não liga o daemon). O **Status** na UI ("Rodando"/"Inativo") deriva de a config
> do OPL estar aplicada (conf isolado + `include`), não do estado do daemon.

## Mapa: o que este roteiro fecha
Critérios ainda abertos em `plans/fase-2-biblioteca.md`:

| Critério pendente | Onde validar aqui |
|---|---|
| Capa baixada por Game ID, gravada em `ART/`, **OPL reconhece** | Parte 3 |
| Autenticação real (cliente SMB com usuário/senha) | Parte 2 |
| `reload smbd` aplica share novo (inclusive `[global]` NT1) | Parte 4.1 |
| `opl_share.conf` criado sob `pkexec` legível (0644) p/ status sem root | Parte 4.2 |

## Pré-requisitos
- Máquina Linux com o oplhost (host do share). Samba 4.23.x instalado e o `smbd`
  **já habilitado** pelo sistema (`systemctl is-active smbd`). Se estiver parado,
  habilitá-lo antes — o app não liga o daemon (ver nota do modelo acima).
- Um **cliente** na mesma rede local. Qualquer um destes:
  - outra máquina Linux com `smbclient`;
  - um PC Windows (Explorer → `\\<IP>\PS2SMB`);
  - o **PS2 + OPL** (validação definitiva).
- Saber o IP local do host (a UI mostra em "IP local"). Neste ambiente: usuário
  do sistema = `maicom` (vira o usuário do share autenticado).
- Pasta-raiz do OPL com jogos. Ex.:
  `/home/maicom/Documentos/Arquivado/2025/OPL_BACKUP` (contém `CD/`, `DVD/`, …).

## Comandos de verificação (rodar no host)
```bash
# share isolado existe e o include foi injetado (com o marcador do app)
sudo cat /etc/samba/opl_share.conf
grep -n 'oplhost\|opl_share.conf' /etc/samba/smb.conf

# o Samba aceita o conf (deve dizer "Loaded services file OK")
testparm -s /etc/samba/opl_share.conf

# o smbd está escutando na 445
ss -ltnp | grep ':445'

# o share aparece na lista
smbclient -L localhost -N
```

---

## Parte 1 — Acesso guest (padrão)

### 1.1 Aplicar
1. Abrir o oplhost.
2. Em "Diretório-alvo do OPL", escolher a pasta-raiz do OPL (a que **contém**
   `CD/`, `DVD/`, `ART/`…).
3. Deixar o acesso no padrão **guest** — em **⚙ Configurações**, "Exigir usuário
   e senha" **desmarcado** (volte à tela principal com "← Voltar").
4. Clicar **Ativar servidor** → digitar a senha no prompt do Polkit (uma vez).
5. Conferir: Status = "Rodando"; o catálogo lista os jogos; IP local exibido.

### 1.2 Verificar no host
- `testparm -s /etc/samba/opl_share.conf` → "Loaded services file OK", com
  `guest ok = yes` e **sem** `valid users`.
- `ss -ltnp | grep ':445'` → há listener.
- Firewall: se o `ufw` estiver ativo, `sudo ufw status` deve listar `445/tcp ALLOW`.

### 1.3 Conectar de um cliente Linux (guest)
```bash
# listar (sem senha)
smbclient -L //<IP-do-host> -N
# entrar no share e ler/escrever
smbclient //<IP-do-host>/PS2SMB -N
smb: \> ls
smb: \> mkdir teste_oplhost
smb: \> rmdir teste_oplhost
smb: \> quit
```
**Esperado:** lista, cria e remove sem pedir senha. Os arquivos criados pertencem
a `maicom` (efeito do `force user`).

### 1.4 Conectar do OPL (PS2)
1. No OPL: Network config → SMB.
2. IP do servidor = IP do host; porta 445.
3. Usuário/senha: deixar guest (em geral usuário `guest`/vazio, senha vazia).
4. Share = `PS2SMB`.
5. Salvar e recarregar a lista de jogos.
**Esperado:** o OPL conecta e mostra os jogos de `CD/` e `DVD/`.

### 1.5 Reverter
1. Clicar **Desativar e reverter** → senha no Polkit.
2. Status volta a "Inativo". Verificar limpeza (§0 — sem vestígios):
   - `/etc/samba/opl_share.conf` **não existe** mais;
   - a linha `include = …opl_share.conf` e o marcador `# oplhost` saíram do
     `smb.conf`;
   - se o `ufw` estava ativo, a regra `445/tcp` foi removida.

---

## Parte 2 — Acesso autenticado (usuário/senha)  ← critério "auth real"

### 2.1 Aplicar
1. Em **⚙ Configurações**, marcar **"Exigir usuário e senha"**.
2. Digitar uma senha no campo que aparece (o usuário é `maicom`, mostrado na UI).
3. Voltar ("← Voltar") e clicar **Ativar servidor** → senha no Polkit.

### 2.2 Verificar no host
- `testparm -s /etc/samba/opl_share.conf` → `guest ok = no` e `valid users = maicom`.
- A conta Samba foi criada: `sudo pdbedit -L` deve listar `maicom`.

### 2.3 Conectar de um cliente Linux
```bash
# guest agora deve FALHAR (NT_STATUS_ACCESS_DENIED / logon failure)
smbclient //<IP-do-host>/PS2SMB -N

# com usuário e senha deve FUNCIONAR
smbclient //<IP-do-host>/PS2SMB -U maicom
# (digitar a senha definida na UI)
smb: \> ls
smb: \> quit
```
**Esperado:** guest é recusado; `-U maicom` + senha entra e lista. **Este é o
critério "auth real" pendente** — anotar o resultado literal.

### 2.4 Conectar do OPL (PS2)
1. No OPL SMB: usuário = `maicom`, senha = a definida na UI.
2. Share = `PS2SMB`; recarregar a lista.
**Esperado:** conecta com as credenciais; sem elas, recusa.

### 2.5 Reverter
1. **Desativar e reverter** → senha no Polkit.
2. Verificar:
   - `sudo pdbedit -L` **não lista mais** `maicom` (o `smbpasswd -x` rodou);
   - conf isolado, include e marcador removidos como na Parte 1.

---

## Parte 3 — Capas / ART  ← critério "capa por Game ID + OPL reconhece"

> O catálogo já mostra título/ID/mídia/tamanho. Falta validar o caminho de
> **download de capa** e o **reconhecimento pelo OPL**. Sem scraping: só fontes
> consumíveis (§7).

### 3.1 Baixar
1. Carregar a pasta-alvo (catálogo populado; o botão "Baixar capas" só habilita
   com catálogo carregado).
2. Clicar **Baixar capas**. Aguardar (a worker thread baixa sem travar a UI).
   - Lembrar do risco conhecido: archive.org pode dar **503 intermitente** — o
     `ArtProvider` faz retry/backoff e falha graciosa. Se falhar tudo, repetir ou
     apontar uma base URL/mirror (ver risco no plano).

### 3.2 Verificar no host
```bash
ls -la <pasta-alvo>/ART/
```
**Esperado:** arquivos nomeados por **Game ID + sufixo do OPL**, ex.:
`SLUS_200.02_COV.png` (capa frente), `_COV2` (verso), `_ICO`, `_BG` etc. — não
rebaixa o que já existe (cache).

### 3.3 Reconhecimento pelo OPL
1. Com o servidor ativo e o OPL conectado, recarregar a lista de jogos.
2. Navegar até um jogo cuja capa foi baixada.
**Esperado:** o OPL exibe a **capa** (não o placeholder). Anotar quais Game IDs
renderizaram e quais não (capa ausente na fonte ≠ bug).

---

## Parte 4 — Comportamento do daemon e legibilidade

### 4.1 `reload` aplica share novo  ← critério "reload smbd"
Objetivo: confirmar que `systemctl reload smbd` (não `restart`) é suficiente para
o Samba alvo passar a servir o share novo, **inclusive as mudanças no bloco
`[global]`** (forçar NT1).
1. Com o `smbd` já ativo e **sem** config do app aplicada, ativar o servidor pelo
   app (que faz só `reload`).
2. Imediatamente conectar um cliente **legado/NT1** (o próprio OPL, ou
   `smbclient ... -m NT1`). 
**Esperado:** conecta via NT1 já após o reload — sem precisar de um `restart`
manual. Se **não** conectar sem restart, registrar a versão exata do Samba: pode
ser um caso em que mudança de protocolo no `[global]` exige restart nessa versão
(reabrir o item no plano).
3. Sanidade do pressuposto: parar o `smbd` (`sudo systemctl stop smbd`) e tentar
   ativar pelo app. **Esperado:** o app não liga o daemon; documentar a mensagem
   ao usuário nesse cenário (o app pressupõe Samba habilitado).

### 4.2 `opl_share.conf` legível sem root (0644)  ← critério "status sem root"
O `status()` lê `opl_share.conf` + `smb.conf` **sem privilégio** (assume arquivos
world-readable). Confirmar que o conf criado sob `pkexec` nasce 0644.
```bash
# com o servidor ativo:
stat -c '%a %n' /etc/samba/opl_share.conf      # esperado: 644
# e que um usuário SEM sudo consegue ler (o status depende disso):
cat /etc/samba/opl_share.conf >/dev/null && echo "legível sem root: OK"
```
**Esperado:** permissões `644` e leitura sem `sudo`. Se vier `600`/sem leitura, o
status não detecta o servidor para o usuário comum → ajustar o `umask`/`chmod` no
script de apply.

---

## O que registrar
Para cada parte, anotar: conectou? leu? escreveu? guest foi recusado no modo
autenticado? a capa apareceu no OPL? o reload bastou (sem restart)? o conf ficou
644? a reversão limpou tudo? Versão do Samba do host e do OPL/BDM no PS2. Qualquer
mensagem de erro literal (porta ocupada, Polkit negado, logon failure, 503 no
art). Levar o resultado de volta para `plans/fase-2-biblioteca.md` (marcar os
critérios) antes de abrir o PR para `main`.
