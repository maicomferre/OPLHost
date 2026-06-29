# Política de versões e processo de release

Este projeto distribui pacotes `.deb` pelas
[Releases do GitHub](https://github.com/maicomferre/OPLHost/releases). Não há
repositório APT remoto.

## Versionamento

Adota-se [Semantic Versioning](https://semver.org) com identificador de
pré-lançamento. **Todas as versões são alpha**: o software está em evolução e
nenhuma release é considerada estável. No GitHub, toda release é publicada como
*pre-release*.

Formato da tag (sempre com prefixo `v`):

```
vMAJOR.MINOR.PATCH-alpha.N
```

- **Primeira release:** `v0.1.0-alpha.1` (o valor inicial padronizado).
- **N** é o contador de builds alpha de uma mesma versão-alvo.

### Como incrementar

| Situação | Como muda | Exemplo |
|----------|-----------|---------|
| Nova build alpha da mesma versão-alvo | incrementa `N` | `v0.1.0-alpha.1` → `v0.1.0-alpha.2` |
| Nova funcionalidade / conclusão de fase | sobe `MINOR`, reinicia `N` | `v0.1.0-alpha.x` → `v0.2.0-alpha.1` |
| Correção pontual sobre uma versão-alvo já aberta | sobe `PATCH`, reinicia `N` | `v0.1.0-alpha.1` → `v0.1.1-alpha.1` |

Enquanto o projeto estiver pré-1.0, `MAJOR` permanece `0`.

### Versão do crate e nome do pacote

A `version` do crate publicado (`crates/ui/Cargo.toml`, pacote `oplhost`) **deve
casar com a tag**, sem o `v`. Ex.: tag `v0.1.0-alpha.1` ⇒ `version =
"0.1.0-alpha.1"`. O workflow de release valida isso e falha se divergir.

O `cargo-deb` converte o pré-lançamento SemVer (`-alpha.N`) para o formato Debian
(`~alpha.N`, que ordena **antes** da versão final). O arquivo gerado fica como:

```
oplhost_<versão>~alpha.<N>-<revisão>_amd64.deb
```

por exemplo `oplhost_0.1.0~alpha.1-1_amd64.deb`.

## Processo de release

1. Atualize a `version` em `crates/ui/Cargo.toml` para `X.Y.Z-alpha.N`.
2. Acrescente a entrada correspondente em `crates/ui/packaging/changelog`.
3. Abra um PR com essas mudanças e faça o merge na `main` (a `main` só recebe via
   PR).
4. Na `main` atualizada, crie e publique a tag anotada:
   ```bash
   git tag -a v0.1.0-alpha.1 -m "oplhost v0.1.0-alpha.1"
   git push origin v0.1.0-alpha.1
   ```
5. O workflow `.github/workflows/release.yml` dispara com a tag, gera o `.deb` e
   publica a Release (marcada como *pre-release*), anexando o pacote.
