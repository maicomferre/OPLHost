# plans/ — planos de desenvolvimento do `oplhost`

Cada fase do roadmap tem um plano próprio aqui, seguindo o
[`PLANS_TEMPLATE.md`](PLANS_TEMPLATE.md). O `CLAUDE.md` na raiz é a fonte de
verdade das **regras** atuais; estes arquivos guardam o **histórico de decisões**
(o *porquê*) e o andamento de cada fase. Sempre que um plano mudar, commite.

## Índice de planos

| Fase | Plano | Status |
|------|-------|--------|
| 0 | [fase-0-spike.md](fase-0-spike.md) — spike de validação SMBv1 | ✅ Concluído |
| 1 | [fase-1-nucleo.md](fase-1-nucleo.md) — núcleo funcional (core/infra/ui) | ✅ Concluído |
| 2 | [fase-2-biblioteca.md](fase-2-biblioteca.md) — biblioteca, art por Game ID, auth opcional | ✅ Concluído (validado em campo) |
| 3 | [fase-3-i18n.md](fase-3-i18n.md) — i18n pt-BR/en-US (híbrido) | ✅ Concluído |
| 3 | [fase-3-persistencia-ui.md](fase-3-persistencia-ui.md) — persistência XDG + ícone/.deb | ✅ Persistência ok; 🚧 ícone a validar |
| 3 | [fase-3-editor-metadados.md](fase-3-editor-metadados.md) — editor de info (`CFG/<id>.cfg`) | 🚧 Falta validar no OPL real |

Documento de apoio (não é fase):

| Doc | Conteúdo |
|-----|----------|
| [roteiro-teste-manual-share.md](roteiro-teste-manual-share.md) | 📋 Roteiro reutilizável de teste manual do share SMB |

Itens da Fase 3 ainda sem plano próprio (ganham um quando entrarem em foco):
tray (SNI/`ksni`), `UdpbdBackend`, FTP remoto, listas de compatibilidade.
