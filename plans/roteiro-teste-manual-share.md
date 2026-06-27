# Roteiro de teste manual — share SMB (guest + autenticado)

> Validação fim-a-fim do servidor SMBv1 gerado pelo oplhost: acesso **guest**
> (padrão) e acesso **autenticado** (usuário/senha). Cobre o que os testes
> automatizados não alcançam: conexão real de um cliente e do OPL no PS2.
>
> **Atenção (limitação conhecida, ver feedback da Fase 2):** hoje o app dá
> `systemctl start/stop smbd` global. Se você usa Samba para **outras** coisas
> nesta máquina, "Parar e reverter" derruba o smbd inteiro. Faça este teste numa
> máquina onde o Samba seja dedicado ao PS2, ou esteja ciente do efeito.

## Pré-requisitos
- Máquina Linux com o oplhost (host do share). Samba 4.23.x instalado.
- Um **cliente** na mesma rede local. Qualquer um destes:
  - outra máquina Linux com `smbclient`;
  - um PC Windows (Explorer → `\\<IP>\PS2SMB`);
  - o **PS2 + OPL** (validação definitiva).
- Saber o IP local do host (a UI mostra em "IP local"). Neste ambiente: usuário
  do sistema = `maicom` (vira o usuário do share autenticado).

## Comandos de verificação (rodar no host)
```bash
# share isolado existe e o include foi injetado
sudo cat /etc/samba/opl_share.conf
grep -n 'opl_share.conf' /etc/samba/smb.conf

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
   `CD/`, `DVD/`, `ART/`…). Ex.: `/home/maicom/Documentos/Arquivado/2025/OPL_BACKUP`.
3. Deixar "Exigir usuário e senha" **desmarcado**.
4. Clicar **Iniciar servidor** → digitar a senha no prompt do Polkit (uma vez).
5. Conferir: Status = "Rodando"; o catálogo lista os jogos.

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
1. Clicar **Parar e reverter** → senha no Polkit.
2. Verificar limpeza (§0 — sem vestígios):
   - `/etc/samba/opl_share.conf` **não existe** mais;
   - a linha `include = …opl_share.conf` e o marcador `# oplhost` saíram do
     `smb.conf`;
   - se o `ufw` estava ativo, a regra `445/tcp` foi removida.

---

## Parte 2 — Acesso autenticado (usuário/senha)

### 2.1 Aplicar
1. Marcar **"Exigir usuário e senha"**.
2. Digitar uma senha no campo que aparece (o usuário é `maicom`, mostrado na UI).
3. Clicar **Iniciar servidor** → senha no Polkit.

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
**Esperado:** guest é recusado; `-U maicom` + senha entra e lista.

### 2.4 Conectar do OPL (PS2)
1. No OPL SMB: usuário = `maicom`, senha = a definida na UI.
2. Share = `PS2SMB`; recarregar a lista.
**Esperado:** conecta com as credenciais; sem elas, recusa.
> _Pendência declarada: este caminho (senha via OPL) ainda não foi validado._

### 2.5 Reverter
1. **Parar e reverter** → senha no Polkit.
2. Verificar:
   - `sudo pdbedit -L` **não lista mais** `maicom` (o `smbpasswd -x` rodou);
   - conf isolado, include e marcador removidos como na Parte 1.

---

## O que registrar
Para cada parte, anotar: conectou? leu? escreveu? guest foi recusado no modo
autenticado? a reversão limpou tudo? Versão do Samba do host e do OPL/BDM no PS2.
Qualquer mensagem de erro literal (porta ocupada, Polkit negado, logon failure).
