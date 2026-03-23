#!/usr/bin/env bash
set -euo pipefail

FRAMEWORK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$FRAMEWORK_ROOT/.." && pwd)"

SOURCE_DIR="$REPO_ROOT/tooling/pitgun_simulator/data"
DEST_DIR="$FRAMEWORK_ROOT/crates/pitgun-simulator/data"

CATEGORIES=(
  aero
  chassis
  circuits
  drivers
  engines
  tires
  vehicles
)

if [ ! -d "$SOURCE_DIR" ]; then
  echo "error: source data pack not found: $SOURCE_DIR" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"

echo "Syncing simulator data pack"
echo "  source: $SOURCE_DIR"
echo "  dest:   $DEST_DIR"

for category in "${CATEGORIES[@]}"; do
  src="$SOURCE_DIR/$category"
  dst="$DEST_DIR/$category"

  mkdir -p "$dst"

  find "$dst" -maxdepth 1 -type f -name '*.json' -delete

  if [ -d "$src" ]; then
    find "$src" -maxdepth 1 -type f -name '*.json' -exec cp {} "$dst"/ \;
  fi
done

echo "Simulator data pack synchronized."
