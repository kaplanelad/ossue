#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <new-version>"
  echo "Example: $0 0.1.1"
  exit 1
fi

NEW_VERSION="$1"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

FILES=(
  "package.json"
  "crates/core/Cargo.toml"
  "src-tauri/Cargo.toml"
  "src-tauri/tauri.conf.json"
)

for file in "${FILES[@]}"; do
  filepath="$ROOT_DIR/$file"
  if [ ! -f "$filepath" ]; then
    echo "Warning: $file not found, skipping"
    continue
  fi

  case "$file" in
    *.json)
      # Replace "version": "x.y.z" (first occurrence only)
      awk -v new="$NEW_VERSION" '!done && /"version":/ { sub(/"version": "[^"]*"/, "\"version\": \"" new "\""); done=1 } 1' "$filepath" > "$filepath.tmp" && mv "$filepath.tmp" "$filepath"
      ;;
    *.toml)
      # Replace version = "x.y.z" (first occurrence only)
      awk -v new="$NEW_VERSION" '!done && /^version = / { sub(/^version = "[^"]*"/, "version = \"" new "\""); done=1 } 1' "$filepath" > "$filepath.tmp" && mv "$filepath.tmp" "$filepath"
      ;;
  esac

  echo "Updated $file -> $NEW_VERSION"
done

echo "Done! Version bumped to $NEW_VERSION"
