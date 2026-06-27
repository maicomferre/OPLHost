# Fase 0 — Spike de validação SMBv1

> Gate obrigatório do projeto. Um binário DESCARTÁVEL (`spike/`), sem
> arquitetura, que prova que o Open PS2 Loader consegue conectar via SMBv1
> (NT1). Enquanto isto não estiver provado com um cliente real, nada da
> arquitetura definitiva é considerado válido.

- **Status:** Concluído
- **Criado em:** 2026-06-26
- **Última atualização:** 2026-06-26 (PS2 real confirmado; spike removido)

## Contexto e objetivo
O OPL só fala SMBv1 (CIFS/NT1), protocolo que o Samba moderno desabilita por
padrão. Confirmado no ambiente: Samba `4.23.6` com `server min protocol` e
`client min protocol` em `SMB2_02` por padrão — ou seja, SMBv1 desligado.
Objetivo da fase: provar que reabilitar NT1 num share isolado permite conexão
guest, antes de investir em qualquer arquitetura.

## Escopo
- **Dentro:** geração do `opl_share.conf` (SMBv1), injeção idempotente do
  `include` no `smb.conf`, restart do `smbd`, abertura da porta 445, tudo numa
  única janela `pkexec`; comando de `rollback` que reverte; validação local via
  `smbclient` forçando NT1.
- **Fora:** qualquer Trait, camada, GUI ou persistência. Nada de arquitetura.
  O spike é jogado fora após a aprovação.

## Decisões
| Data | Decisão | Justificativa | Alternativas consideradas |
|------|---------|---------------|---------------------------|
| 2026-06-26 | Spike em `spike/`, projeto Cargo próprio e fora do workspace | Deixa explícito que é throwaway; some sem afetar a arquitetura | Módulo dentro do crate principal (acoplaria o descartável ao definitivo) |
| 2026-06-26 | Agrupar todas as operações root num único script via um `pkexec` | Evita múltiplos prompts de senha; espelha a regra de "janela de privilégio" do projeto | Um `pkexec` por operação (pediria senha várias vezes) |
| 2026-06-26 | Share isolado em `/etc/samba/opl_share.conf` + 1 linha de `include`; nunca editar o conteúdo global | Rollback trivial (remover arquivo + linha); não corrompe o smb.conf do usuário | Editar `[global]` diretamente no `smb.conf` |
| 2026-06-26 | Validação local com `smbclient` forçando `client min/max protocol = NT1` | Prova o lado servidor sem depender do PS2; sinal local mais forte possível | Só inspecionar `testparm` (não prova conexão real) |

## A validar no ambiente
- [x] `testparm -s /etc/samba/opl_share.conf` carrega sem erro fatal. Avisos
      esperados confirmados: `"lanman auth" option is deprecated` e `Weak crypto
      is allowed by GnuTLS (e.g. NTLM as a compatibility fallback)`. O Samba
      traduz `ntlm auth = yes` para `ntlm auth = ntlmv1-permitted`.
- [x] Após `apply`, `server min protocol` reportado como `NT1`.
- [x] `smbclient` NT1 guest **conecta, lista E escreve** no share (`Anonymous
      login successful`; `put` de arquivo de teste OK; `force user = maicom`
      aplicado — arquivo gravado como `maicom`).
- [x] Conexão de um **PS2 com OPL real** ao share (porta 445, IP local)
      confirmada pelo usuário em 2026-06-26. **Premissa SMBv1 do projeto provada.**

### Resultado da validação local (2026-06-26)
Ambiente: Samba 4.23.6. O esqueleto de config do spike **funciona sem ajustes**:
o servidor passou a aceitar conexão SMBv1 (NT1) guest com leitura e escrita. Os
avisos de crypto fraca são esperados e fazem parte do trade-off do OPL.

## Tarefas
- [x] Escrever o spike (`apply`/`rollback`).
- [x] Compilar o spike isoladamente.
- [x] Rodar `apply` e validar localmente com `smbclient` (NT1).
- [x] Registrar o resultado da validação local aqui.
- [x] Usuário confirma conexão do PS2 real.
- [x] Após confirmação: rodar `rollback`, remover `spike/` e marcar a fase como
      Concluída.

## Critérios de aceitação
- [x] `smbclient` NT1 local conecta e lista o share.
- [x] `rollback` retorna o sistema ao estado anterior (verificado: sem
      `opl_share.conf`, sem linha de include, `server min protocol` de volta a
      `SMB2_02`, regra de firewall removida).
- [x] Usuário confirma que o PS2/OPL real conecta.

## Riscos e mitigação
- **Risco:** Samba 4.23.6 rejeitar/avisar params legados (`lanman auth`,
  `ntlm auth`). → **Mitigação:** ajustar o esqueleto no spike até o `smbclient`
  NT1 conectar; é o objetivo do spike descobrir os valores exatos.
- **Risco:** porta 445 ocupada por outro serviço. → **Mitigação:** o spike
  reporta o erro; OPL aceita porta alternativa (a tratar na arquitetura).

## Histórico
| Data | Mudança | Commit |
|------|---------|--------|
| 2026-06-26 | Spike criado e compilando; plano da fase aberto | `b8e355e` |
| 2026-06-26 | Validação local OK (NT1 guest lê/escreve); pendente só o PS2 real | `71adb33` |
| 2026-06-26 | PS2 real confirmado; rollback verificado; spike removido; fase Concluída | _(pendente)_ |
