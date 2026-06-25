# PHPM

*[Leia em português](README-pt.md)*

**A PHP dependency manager with a shared global store.** A compatibility layer over Composer, the way pnpm was for npm.

PHPM does not replace Composer. It reuses your `composer.json`, your `composer.lock`, and Composer's own solver, and swaps out the expensive part: instead of every project carrying its own byte-for-byte copied `vendor/`, PHPM stores each `(package, version)` **once** in a global store and materializes each project's `vendor/` with **file-by-file hard links**.

```bash
cd your-php-project     # already has composer.json + composer.lock
phpm install            # vendor/ materialized via hard links from the global store
php artisan serve       # Laravel/Symfony/etc. boot with no changes to the project
```

---

## Motivation

On a dev machine (or a CI runner) with several PHP projects, the disk fills up with identical copies:

```
crm/vendor       500 MB
erp/vendor       480 MB
api/vendor       430 MB
landing/vendor   400 MB
-----------------------
total          ~1.8 GB
```

Most of those files are **byte-for-byte identical**: `monolog/monolog 3.8.1`, the `symfony/*` components, `guzzlehttp/guzzle`, the PSR packages. Each one is replicated in every project that uses it.

Composer has a **download** cache (`~/.composer/cache`), which avoids re-downloading, but it does **not** avoid re-extracting or the duplication on materialized disk. Every `composer install` extracts repeated gigabytes.

PHPM stores each `(package, version)` once in the global store and materializes `vendor/` via hard links, which consume no extra data space (only directory inodes). The result:

- **Disk**: N projects with the same packages take ~1 copy, not N.
- **Speed**: with a warm store, materializing `vendor/` is hard linking in seconds, with no download and no extraction.
- **Compatibility**: to PHP, each file is indistinguishable from a plain copy. `realpath()` resolves inside the project's `vendor/`, it does not leak to the store. Laravel and Symfony work unchanged.

The gain is sharpest **where there are many projects**: dev shops, separate monorepos, and above all **CI/fleets**. Installing 20 Laravel apps on a runner today extracts repeated gigabytes; with PHPM it is a fraction of the disk and time.

---

## How it works

```
phpm install
   │
   ├─ read composer.json + composer.lock
   │     (no lock → delegate to Composer: composer update --no-install)
   │
   ├─ acquire   download each package (dist zip / git source), verify integrity,
   │            extract into the global store ONCE
   │
   ├─ linker    materialize vendor/<vendor>/<package>/ via file-by-file hard links
   │            (idempotent sync: add missing, remove stale; never duplicate data)
   │
   ├─ compat    generate vendor/autoload.php + vendor/composer/* + vendor/bin
   │            (Composer-compatible: installed.json/php, ClassLoader, bin proxies)
   │
   └─ scripts   composer run-script post-autoload-dump
                (this is where Laravel's package:discover registers service providers)
```

Composer **never touches `vendor/`**: it only resolves (`--no-install`) and runs scripts. All materialization is PHPM's. That boundary is what makes the speed gain real.

### Core decision: file-by-file hard links

A hard link operates on **files**, not directories. For each package, PHPM recreates the directory tree under `vendor/` (negligible cost, just empty inodes) and makes **each file** a hard link to the matching file in the store. The content, which is what weighs, is never duplicated.

We do not use directory symlinks (the way pnpm does on Node) because PHP is sensitive to `realpath()`: Laravel and Symfony call `realpath()` to locate config, views, migrations, and service providers, and a directory symlink would make that leak to the store. A file-by-file hard link is indistinguishable from a copy to PHP.

### Immutable global store

```
~/.phpm/store/
  packages/<vendor>/<package>/<version>/   ← extracted content, read-only
  meta/<vendor>/<package>/<version>.json   ← {name, version, sha256}
```

The store is **read-only** after it is written. Because the files in `vendor/` are the *same inode* as the store, an accidental write to `vendor/` becomes a loud error instead of silently corrupting the global store shared by every project. Writes are atomic (temp, fsync, rename) and have a per-package concurrency lock.

---

## Commands

```bash
phpm install            # materialize vendor/ from composer.lock
phpm install --no-dev   # skip require-dev (production deploy)

phpm require monolog/monolog:^3.0   # add a dependency (Composer resolves) + install
phpm remove monolog/monolog         # remove a dependency + re-sync vendor/
phpm update                         # re-resolve the lock + install

phpm gc                 # show what it would remove from the store (dry run, safe default)
phpm gc --prune         # actually remove packages no project references
```

`require`/`remove`/`update` delegate the lock mutation to Composer (`--no-install`) and then run the same idempotent `install` pipeline.

---

## Installation

### Quick (prebuilt binary)

```bash
# macOS and Linux
curl -LsSf https://github.com/lemesdaniel/phpm/releases/latest/download/install.sh | sh

# Windows (PowerShell)
powershell -ExecutionPolicy ByPass -c "irm https://github.com/lemesdaniel/phpm/releases/latest/download/install.ps1 | iex"
```

The script detects OS/architecture, downloads the binary from GitHub Releases, and installs it to `~/.local/bin` (or `%LOCALAPPDATA%\phpm\bin` on Windows). Override the destination with `PHPM_INSTALL_DIR` and pin a version with `PHPM_VERSION=v0.1.0`.

### From source

```bash
cargo build --release -p cli
cp target/release/phpm /usr/local/bin/phpm   # or another dir on PATH
```

**Prerequisites (all modes):** `composer` (2.x), `php` (8.x), and `git` on PATH. PHPM uses Composer to resolve versions (a deliberate v1 decision) and `git` for packages with a git source.

---

## Migrating a Composer project

There is no migration: PHPM reads the same files:

```bash
cd composer-project        # has composer.json + composer.lock
rm -rf vendor              # optional
phpm install               # rebuild vendor/ from the same lock
```

No `phpm.json`, no separate lock. Reversible at any time: `composer install` rebuilds a normal `vendor/`. Both read the same `composer.json`/`composer.lock`.

---

## Store on a separate volume (CI, Docker)

A hard link **cannot cross a filesystem**. The store and the project's `vendor/` must be on the same volume. Configure it:

```bash
PHPM_STORE_DIR=/workspace/volume/.phpm-store phpm install
```

If the store lands on a different volume, PHPM **warns** and falls back to copying (loses disk dedup, keeps part of the speed gain with a warm store). On CI/runners, point `PHPM_STORE_DIR` at the same volume as the workspaces.

In Docker, dedup works within a single layer (store + `/app` on the same overlay fs). `--mount=type=cache` for the store gives a warm store across builds, but it is a separate mount, so it copies. Multi-stage `COPY --from` materializes real bytes (the final image has a normal-sized `vendor/`). PHPM's disk gain is mostly a **build/CI** benefit, not a final-image-size one.

---

## Status and limitations (v1 / MVP)

Validated against real frameworks: **Laravel 13** (`artisan` boots, package discovery), **Symfony 8.1** (`bin/console`), **PHPUnit** (`vendor/bin/phpunit`).

v1 is, deliberately, an *install accelerator + disk deduplicator* built on top of Composer. Known limitations:

- **Requires PHP + Composer installed** (v1 has no solver of its own).
- **`post-install-cmd` / `post-update-cmd` are not run**, only `post-autoload-dump`. On a new project, run `php artisan key:generate` / `storage:link` manually once.
- **`path` repositories** (local packages) are not yet supported.
- **Composer event plugins** (those hooking script events such as `post-autoload-dump`) run through Composer when the project lists them in `config.allow-plugins`. **Installer plugins** that relocate install paths are not honored, because phpm (not Composer) materializes `vendor/`.
- **Packages that write into their own `vendor/`** (rare) fail loudly because of the read-only store.

---

## Architecture (Rust crates)

```
crates/
  lockfile/         parse composer.json / composer.lock (pure, no I/O)
  store/            global store: layout, atomic writes, integrity, locks
  acquire/          download dist + clone git source → store
  linker/           hard link store → vendor/ (idempotent sync)
  compat_composer/  generate autoload + installed.json/php + bin proxies
  composer_bridge/  bridge to the Composer CLI (resolve --no-install, run-script)
  gc/               store garbage collection + project registry
  cli/              the `phpm` binary (5 commands)
```

See `AGENTS.md` for build, tests, and contribution conventions.

## License

MIT.
