# Mensagens DINÂMICAS montadas em Rust (status, erros, progresso, resumo, dicas).
# Idioma-fonte (en-US). pt-BR.ftl deve ter EXATAMENTE as mesmas chaves
# (garantido por teste de paridade em i18n.rs). Placeholders fluent: { $nome }.

ip-unavailable = unavailable (offline?)
catalog-empty = No directory loaded.

status-active = Active — sharing the OPL catalog
status-inactive = Inactive — configuration not applied

catalog-summary = { $total } game(s) — { $cd } CD, { $dvd } DVD — { $size }

# Dicas do diretório-alvo
hint-empty = The OPL folder structure (CD, DVD, ART…) is created here if it doesn't exist yet.
hint-parent-fallback = the folder above
hint-subdir = You selected the "{ $name }" subfolder of the OPL structure. The OPL root is probably "{ $parent }" — select it.
hint-detected = OPL structure detected in this folder — nothing will be recreated.
hint-will-create = The OPL folder structure (CD, DVD, ART…) will be created here, since it doesn't exist yet.

# Progresso (operações em background)
progress-applying = Applying configuration (enter your password at the prompt)…
progress-reverting = Reverting configuration (enter your password at the prompt)…
progress-selecting-folder = Selecting folder…
progress-downloading-art = Downloading covers from external sources…

# Validações / mensagens
msg-choose-dir-before-activate = Choose a target directory before activating.
msg-set-password = Set a password for authenticated access (or uncheck the option).
msg-choose-dir-before-art = Choose a target directory before downloading covers.
msg-cannot-create-dir = Could not create { $path }: { $error }
msg-covers-result = Covers — { $downloaded } downloaded, { $skipped } already existed, { $notfound } missing at source, { $noid } without Game ID, { $errors } network error(s).
msg-cannot-read-meta = Could not read the metadata: { $error }
msg-no-game-id = Game has no Game ID — nowhere to write.
msg-meta-saved = Metadata saved to CFG/{ $id }.cfg.
msg-cannot-save-meta = Could not save — { $error }
msg-cannot-create-layout = Failed to create the structure in { $path }: { $error }
msg-cannot-activate = Could not activate: { $error }
msg-reverted = Configuration reverted. Nothing from the app remains on the system.
msg-cannot-revert = Failed to revert: { $error }
