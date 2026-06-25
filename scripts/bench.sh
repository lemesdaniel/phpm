#!/bin/sh
# Anchor benchmark: N projects. Composer keeps a full vendor/ per project; phpm shares one
# global store and hard-links each vendor/. Measures disk footprint and warm install time.
# Usage: scripts/bench.sh [N]   (default N=8). Needs composer + php.
set -eu
N="${1:-8}"
ROOT="$(mktemp -d)"
STORE="$ROOT/store"
echo "phpm anchor benchmark: N=$N projects"
echo "workdir: $ROOT"

cargo build --release -p cli >/dev/null 2>&1
PHPM="$(pwd)/target/release/phpm"

# Template: a representative dependency set, resolved once (lock reused by all projects).
TPL="$ROOT/tpl"
mkdir -p "$TPL"
cat > "$TPL/composer.json" <<'JSON'
{ "name": "bench/app", "require": { "monolog/monolog": "^3.0", "guzzlehttp/guzzle": "^7.0", "symfony/console": "^7.0" } }
JSON
( cd "$TPL" && composer update --no-install --no-scripts --no-interaction >/dev/null 2>&1 )

# Composer: N independent vendor/ trees
COMPOSER_DIR="$ROOT/composer"
c0=$(date +%s)
i=0; while [ "$i" -lt "$N" ]; do
  d="$COMPOSER_DIR/p$i"; mkdir -p "$d"; cp "$TPL/composer.json" "$TPL/composer.lock" "$d/"
  ( cd "$d" && composer install --no-scripts --no-interaction >/dev/null 2>&1 )
  i=$((i+1))
done
c1=$(date +%s)
# BSD du on macOS: -m flag for megabytes; fall back to -k and divide if needed
if du -sm "$COMPOSER_DIR" >/dev/null 2>&1; then
  composer_disk=$(du -sm "$COMPOSER_DIR" | cut -f1)
else
  composer_disk=$(du -sk "$COMPOSER_DIR" | awk '{printf "%d", $1/1024}')
fi

# phpm: N vendor/ sharing one store. Warm the store with p0 (cold), then time p1..N-1 (warm).
PHPM_DIR="$ROOT/phpm"
mkdir -p "$PHPM_DIR/p0"; cp "$TPL/composer.json" "$TPL/composer.lock" "$PHPM_DIR/p0/"
PHPM_STORE_DIR="$STORE" sh -c "cd '$PHPM_DIR/p0' && '$PHPM' install >/dev/null 2>&1"
w0=$(date +%s)
i=1; while [ "$i" -lt "$N" ]; do
  d="$PHPM_DIR/p$i"; mkdir -p "$d"; cp "$TPL/composer.json" "$TPL/composer.lock" "$d/"
  PHPM_STORE_DIR="$STORE" sh -c "cd '$d' && '$PHPM' install >/dev/null 2>&1"
  i=$((i+1))
done
w1=$(date +%s)
if du -sm "$PHPM_DIR" >/dev/null 2>&1; then
  phpm_vendor_disk=$(du -sm "$PHPM_DIR" | cut -f1)
  store_disk=$(du -sm "$STORE" | cut -f1)
else
  phpm_vendor_disk=$(du -sk "$PHPM_DIR" | awk '{printf "%d", $1/1024}')
  store_disk=$(du -sk "$STORE" | awk '{printf "%d", $1/1024}')
fi
phpm_total=$((phpm_vendor_disk + store_disk))

echo
echo "RESULTS (N=$N)"
echo "  composer:  ${composer_disk} MB vendor (all projects), $((c1-c0)) s total install"
echo "  phpm:      ${phpm_vendor_disk} MB vendor (hard links) + ${store_disk} MB store = ${phpm_total} MB footprint"
echo "  phpm warm: $((w1-w0)) s for $((N-1)) warm installs (store already populated)"
if [ "$composer_disk" -gt 0 ]; then
  echo "  disk ratio: phpm footprint is $(( phpm_total * 100 / composer_disk ))% of composer"
fi
echo
echo "workdir kept for inspection: $ROOT"
