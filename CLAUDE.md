# CLAUDE.md

See **[AGENTS.md](AGENTS.md)** for everything: crate map, build/test/lint commands, code conventions, design decisions, and known limitations. It is the single source of truth for working in this repo.

Quick reminders for Claude:

- All four gates must pass before committing: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- Source comments and message strings are **English only**; keep only necessary comments.
- `/RFC-*.md` and `/docs/` are gitignored — never add them to git.
- Composer never touches `vendor/`; PHPM owns it. Don't break that boundary.
