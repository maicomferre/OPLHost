# Tradução pt-BR das mensagens dinâmicas (ver en-US.ftl). Mesmas chaves.

ip-unavailable = indisponível (offline?)
catalog-empty = Nenhum diretório carregado.

status-active = Ativo — compartilhando o catálogo do OPL
status-inactive = Inativo — configuração não aplicada

catalog-summary = { $total } jogo(s) — { $cd } CD, { $dvd } DVD — { $size }

# Dicas do diretório-alvo
hint-empty = A estrutura de pastas do OPL (CD, DVD, ART…) é criada aqui se ainda não existir.
hint-parent-fallback = a pasta acima
hint-subdir = Você selecionou a subpasta "{ $name }" da estrutura do OPL. A raiz do OPL provavelmente é "{ $parent }" — selecione-a.
hint-detected = Estrutura do OPL detectada nesta pasta — nada será recriado.
hint-will-create = A estrutura de pastas do OPL (CD, DVD, ART…) será criada aqui, pois ainda não existe.

# Progresso (operações em background)
progress-reload-catalog = Recarregando o catálogo do último diretório…
progress-applying = Aplicando configuração (informe sua senha no prompt)…
progress-reverting = Revertendo configuração (informe sua senha no prompt)…
progress-selecting-folder = Selecionando pasta…
progress-downloading-art = Baixando capas das fontes externas…

# Validações / mensagens
msg-choose-dir-before-activate = Escolha um diretório-alvo antes de ativar.
msg-set-password = Defina uma senha para o acesso autenticado (ou desmarque a opção).
msg-choose-dir-before-art = Escolha um diretório-alvo antes de baixar capas.
msg-cannot-create-dir = Não foi possível criar { $path }: { $error }
msg-covers-result = Capas — { $downloaded } baixada(s), { $skipped } já existia(m), { $notfound } sem capa na fonte, { $noid } sem Game ID, { $errors } erro(s) de rede.
msg-cannot-read-meta = Não foi possível ler os metadados: { $error }
msg-no-game-id = Jogo sem Game ID — não há onde gravar.
msg-meta-saved = Metadados salvos em CFG/{ $id }.cfg.
msg-cannot-save-meta = Não foi possível salvar — { $error }
msg-cannot-create-layout = Falha ao criar a estrutura em { $path }: { $error }
msg-cannot-activate = Não foi possível ativar: { $error }
msg-reverted = Configuração revertida. Nada do app permanece no sistema.
msg-cannot-revert = Falha ao reverter: { $error }
