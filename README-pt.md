# PHPM

*[Read in English](README.md)*

**Gerenciador de dependências PHP com store global compartilhado.** Uma camada de compatibilidade sobre o Composer, como o pnpm foi para o npm.

PHPM não substitui o Composer. Ele reaproveita o `composer.json`, o `composer.lock` e o próprio solver do Composer, e troca a parte cara: em vez de cada projeto carregar seu próprio `vendor/` copiado byte a byte, o PHPM guarda cada `(pacote, versão)` **uma única vez** num store global e materializa o `vendor/` de cada projeto por **hard links arquivo-a-arquivo**.

```bash
cd seu-projeto-php       # já tem composer.json + composer.lock
phpm install            # vendor/ materializado por hard link a partir do store global
php artisan serve       # Laravel/Symfony/etc. sobem sem nenhuma alteração no projeto
```

---

## Motivação

Numa máquina de desenvolvimento (ou num runner de CI) com vários projetos PHP, o disco enche de cópias idênticas:

```
crm/vendor       500 MB
erp/vendor       480 MB
api/vendor       430 MB
landing/vendor   400 MB
-----------------------
total          ~1.8 GB
```

A maior parte desses arquivos é **byte a byte idêntica**: `monolog/monolog 3.8.1`, os componentes `symfony/*`, `guzzlehttp/guzzle`, os pacotes PSR. Cada um aparece replicado em todo projeto que o usa.

O Composer tem cache de **download** (`~/.composer/cache`), o que evita re-baixar, mas **não** evita re-extrair nem a duplicação no disco materializado. Cada `composer install` extrai gigabytes repetidos.

PHPM armazena cada `(pacote, versão)` uma vez no store global e materializa o `vendor/` por hard link, que não consome espaço de dados adicional (só inodes de diretório). O resultado:

- **Disco**: N projetos com os mesmos pacotes ocupam ~1 cópia, não N.
- **Velocidade**: com o store quente, materializar o `vendor/` é hard link em segundos, sem download e sem extração.
- **Compatibilidade**: para o PHP, cada arquivo é indistinguível de uma cópia comum. `realpath()` resolve dentro do `vendor/` do projeto, não vaza para o store. Laravel e Symfony funcionam sem mudança.

O ganho é mais nítido **onde há muitos projetos**: dev shops, monorepos separados, e principalmente **CI/fleets**. Instalar 20 apps Laravel num runner hoje extrai gigabytes repetidos; com PHPM é uma fração do disco e do tempo.

---

## Como funciona

```
phpm install
   │
   ├─ lê composer.json + composer.lock
   │     (lock ausente → delega ao Composer: composer update --no-install)
   │
   ├─ acquire   baixa cada pacote (dist zip / git source), verifica integridade,
   │            extrai para o store global UMA vez
   │
   ├─ linker    materializa vendor/<vendor>/<pacote>/ por hard link arquivo-a-arquivo
   │            (sync idempotente: adiciona faltantes, remove sobrantes; nunca duplica dados)
   │
   ├─ compat    gera vendor/autoload.php + vendor/composer/* + vendor/bin
   │            (compatível com o Composer: installed.json/php, ClassLoader, bin proxies)
   │
   └─ scripts   composer run-script post-autoload-dump
                (é aqui que o package:discover do Laravel registra os service providers)
```

O Composer **nunca toca o `vendor/`**: ele só resolve (`--no-install`) e roda scripts. Toda a materialização é do PHPM. Essa fronteira é o que torna o ganho de velocidade real.

### Decisão central: hard link arquivo-a-arquivo

Hard link opera em **arquivos**, não em diretórios. Para cada pacote, o PHPM recria a árvore de diretórios em `vendor/` (custo desprezível, só inodes vazios) e faz **cada arquivo** ser um hard link para o arquivo correspondente no store. O conteúdo, que é o que pesa, nunca é duplicado.

Não usamos symlink de diretório (como o pnpm faz no Node) porque o PHP é sensível a `realpath()`: Laravel e Symfony chamam `realpath()` para descobrir config, views, migrations e service providers, e symlink de diretório faria isso vazar para o store. Hard link arquivo-a-arquivo é indistinguível de uma cópia para o PHP.

### Store global imutável

```
~/.phpm/store/
  packages/<vendor>/<pacote>/<versão>/   ← conteúdo extraído, read-only
  meta/<vendor>/<pacote>/<versão>.json   ← {nome, versão, sha256}
```

O store é **read-only** após escrito. Como os arquivos do `vendor/` são o *mesmo inode* do store, isso transforma uma escrita acidental em `vendor/` em erro alto (em vez de corromper silenciosamente o store global de todos os projetos). Escrita é atômica (temp → fsync → rename) e tem lock de concorrência por pacote.

---

## Comandos

```bash
phpm install            # materializa vendor/ a partir do composer.lock
phpm install --no-dev   # pula require-dev (deploy de produção)

phpm require monolog/monolog:^3.0   # adiciona dependência (Composer resolve) + instala
phpm remove monolog/monolog         # remove dependência + re-sincroniza vendor/
phpm update                         # re-resolve o lock + instala

phpm gc                 # mostra o que removeria do store (dry-run, padrão seguro)
phpm gc --prune         # remove de fato pacotes que nenhum projeto referencia
```

`require`/`remove`/`update` delegam a mutação do lock ao Composer (`--no-install`) e depois rodam o mesmo pipeline idempotente de `install`.

---

## Instalação

### Rápida (binário pré-compilado)

```bash
# macOS e Linux
curl -LsSf https://github.com/lemesdaniel/phpm/releases/latest/download/install.sh | sh

# Windows (PowerShell)
powershell -ExecutionPolicy ByPass -c "irm https://github.com/lemesdaniel/phpm/releases/latest/download/install.ps1 | iex"
```

O script detecta SO/arquitetura, baixa o binário do GitHub Releases e instala em `~/.local/bin` (ou `%LOCALAPPDATA%\phpm\bin` no Windows). Sobrescreva o destino com `PHPM_INSTALL_DIR` e fixe uma versão com `PHPM_VERSION=v0.1.0`.

### A partir do código

```bash
cargo build --release -p cli
cp target/release/phpm /usr/local/bin/phpm   # ou outro dir no PATH
```

**Pré-requisitos (todos os modos):** `composer` (2.x), `php` (8.x) e `git` no PATH. PHPM usa o Composer para resolver versões (decisão deliberada da v1) e o `git` para pacotes com source git.

---

## Migrando um projeto Composer

Não há migração: o PHPM lê os mesmos arquivos:

```bash
cd projeto-composer        # tem composer.json + composer.lock
rm -rf vendor              # opcional
phpm install               # reconstrói vendor/ do mesmo lock
```

Sem `phpm.json`, sem lock próprio. Reversível a qualquer momento: `composer install` reconstrói o `vendor/` normal. Os dois leem o mesmo `composer.json`/`composer.lock`.

---

## Store em volume separado (CI, Docker)

Hard link **não cruza filesystem**. O store e o `vendor/` do projeto precisam estar no mesmo volume. Configure:

```bash
PHPM_STORE_DIR=/volume/do/workspace/.phpm-store phpm install
```

Se o store cair num volume diferente, o PHPM **avisa** e cai para cópia (perde a dedup de disco, mantém parte do ganho de velocidade com o store quente). Em CI/runners, aponte `PHPM_STORE_DIR` para o mesmo volume dos workspaces.

No Docker, a dedup funciona dentro de uma layer (store + `/app` no mesmo overlay fs). `--mount=type=cache` para o store dá store quente entre builds, mas é um mount separado → cópia. Multi-stage `COPY --from` materializa bytes reais (a imagem final tem `vendor/` de tamanho normal). O ganho de disco do PHPM é principalmente de **build/CI**, não de tamanho de imagem final.

---

## Estado e limitações (v1 / MVP)

Validado contra frameworks reais: **Laravel 13** (`artisan` sobe, package discovery), **Symfony 8.1** (`bin/console`), **PHPUnit** (`vendor/bin/phpunit`).

A v1 é, deliberadamente, um *acelerador de instalação + deduplicador de disco* construído sobre o Composer. Limitações conhecidas:

- **Requer PHP + Composer instalados** (a v1 não tem solver próprio).
- **`post-install-cmd` / `post-update-cmd` não são executados**, só `post-autoload-dump`. Em projeto novo, rode `php artisan key:generate` / `storage:link` manualmente uma vez.
- **`path` repositories** (pacotes locais) ainda não suportados.
- **Plugins de evento do Composer** (os que se conectam a eventos de script como `post-autoload-dump`) rodam via Composer quando o projeto os lista em `config.allow-plugins`. **Plugins instaladores** que mudam o caminho de instalação não são honrados, porque é o phpm (não o Composer) que materializa o `vendor/`.
- **Pacotes que escrevem no próprio `vendor/`** (raros) falham alto por causa do store read-only.

---

## Arquitetura (crates Rust)

```
crates/
  lockfile/         parsing de composer.json / composer.lock (puro, sem I/O)
  store/            store global: layout, escrita atômica, integridade, locks
  acquire/          download de dist + clone de git source → store
  linker/           hard link store → vendor/ (sync idempotente)
  compat_composer/  geração de autoload + installed.json/php + bin proxies
  composer_bridge/  ponte com o CLI do Composer (resolve --no-install, run-script)
  gc/               garbage collection do store + registry de projetos
  cli/              binário `phpm` (5 comandos)
```

Veja `AGENTS.md` para build, testes e convenções de contribuição.

## Licença

MIT.
