# AGENTS.md

Guidance for AI coding agents and contributors working in this repository.

## What this is

PHPM is a PHP dependency manager written in Rust: a compatibility layer over Composer (pnpm-to-npm style). It reuses `composer.json`/`composer.lock` and Composer's solver, and replaces installation with a shared global store + file-by-file hard links into each project's `vendor/`. See `README-pt.md` for the user-facing overview and motivation.

## Crate map (Cargo workspace, `crates/*`)

| Crate | Responsibility |
|-------|----------------|
| `lockfile` | Parse `composer.json` / `composer.lock` into typed structs. Pure, zero I/O. |
| `store` | Owns `~/.phpm/store`: layout, atomic writes, read-only immutability, sha256 integrity, per-package locks, `list_packages`, `remove_package`. |
| `acquire` | Download dist (zip) or clone git source into the store. Verifies shasum; hardened against zip-slip and git arg/protocol injection. Network isolated behind a `Fetcher` trait. |
| `linker` | Materialize `vendor/` from the store via hard links. Idempotent `sync` with a `.phpm-state` sentinel; cross-volume copy fallback. |
| `compat_composer` | Generate a Composer-compatible `vendor/`: `autoload*.php`, `installed.json/php`, `vendor/bin` proxies. Bundles Composer's `ClassLoader.php`/`InstalledVersions.php` (MIT, see `crates/compat_composer/assets/ASSETS_LICENSE`). |
| `composer_bridge` | Shell out to the `composer` CLI: resolve (`--no-install --no-scripts`) and `run-script`. Process execution behind a `Runner` trait. |
| `gc` | Garbage-collect unreferenced store packages; project registry (`~/.phpm/projects`). |
| `cli` | The `phpm` binary (`install`/`require`/`remove`/`update`/`gc`). Orchestration lives in `cli::install` / `cli::gc_run` (testable); `main.rs` is a thin clap front-end. |

Dependency direction is one-way: `cli` → everything; `gc`/`compat_composer`/`acquire`/`linker` → `store` + `lockfile`; `composer_bridge` → `lockfile`. No cycles.

## Build, test, lint

```bash
cargo build --workspace
cargo test  --workspace                 # unit + integration; ignored tests stay ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                          # write
cargo fmt --all -- --check               # verify

cargo build --release -p cli             # the phpm binary → target/release/phpm
```

All four gates (build, test, clippy `-D warnings`, fmt `--check`) must be green before any commit.

### Ignored tests (real environment)

Some tests need `composer` + `php` + network and are marked `#[ignore]` so they are not CI gates. Run them on demand:

```bash
cargo test --workspace -- --ignored                 # all of them
cargo test -p cli --test acceptance -- --ignored    # Laravel / Symfony / phpunit boot for real
```

`crates/cli/tests/acceptance.rs` drives the real `phpm` pipeline against framework skeletons created by real Composer. The benchmark is `scripts/bench.sh` (anchor disk/speed numbers).

## Code conventions

- **English only** in all source: comments, doc comments, and human-readable message strings (`#[error("...")]`, `panic!`, `assert!` messages, CLI output). The project is open-source. (Commit messages and PR discussion may be Portuguese.)
- **Self-explanatory code; necessary comments only.** Keep `///` API docs and "why" comments (rationale, invariants, security, ordering, platform quirks). Drop comments that restate the next line.
- **Errors**: one `thiserror::Error` enum per crate; wrap upstream errors with `#[from]`. No `unwrap()`/`panic!` in library code paths (tests are fine).
- **Testability via injection**: network (`Fetcher`) and process execution (`Runner`) are traits so logic tests run offline. Prefer this over real subprocess/network in unit tests.
- **Security-sensitive surfaces** (the untrusted-input boundary): `acquire` zip extraction (zip-slip, symlink, alloc caps), git invocation (`--` separators, leading-dash rejection, `protocol.ext.allow=never`), and the read-only store. Do not weaken these without an equivalent guard.
- **The `with_extension` footgun**: never use `Path::with_extension` to append a suffix to a version-bearing filename (e.g. `3.8.1` → `3.8.json`). Build the filename explicitly with `format!`.

## Not committed (gitignored)

`/target`, `/RFC-*.md`, and `/docs/` are gitignored on purpose. The RFC, the implementation plans under `docs/superpowers/plans/`, and result artifacts are local; do not add them to git.

## Key design decisions (context for changes)

- Composer **never** writes to `vendor/`; it only resolves (`--no-install`) and runs scripts (`run-script`). PHPM owns the whole `vendor/`. Preserve this boundary.
- Hard link **file-by-file** (not directory symlink) so `realpath()` resolves inside `vendor/`. Required for Laravel/Symfony compatibility.
- `install` is an **idempotent sync**: it makes `vendor/` match the lock exactly. `require`/`remove`/`update` = mutate the lock via Composer, then run `install`.
- The store is **read-only** and per-`(pkg,ver)` immutable; this is what makes shared hard links safe and the classmap cache sound.
- Functional (not byte-identical) Composer compatibility: Composer embeds a per-project random hash in autoload class names, so byte-equality is impossible. Validate by running real PHP, not by diffing.

## Known limitations / backlog (don't "rediscover" these)

`post-install-cmd`/`post-update-cmd` not run (only `post-autoload-dump`); root `autoload-dev` not aggregated; `path` repositories unsupported; Composer plugins unsupported; classmap tokenizer is line-based (heredoc tracked, block comments not fully); `AUTOLOAD_HASH` is a constant (no two phpm projects in one PHP process); cross-volume = copy (no dedup). The own-solver and content-addressable storage are deliberately deferred to the Stable stage.
