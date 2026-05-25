#!/bin/bash
set -euo pipefail

REPO="ildar-safarov/gutenindex"
TAG="corpus-v1"

CORPUS_DIR="${1:?usage: $0 <corpus_zips_dir>}"

gh release create "$TAG" \
    --repo "$REPO" \
    --title "Project Gutenberg corpus" \
    --notes "Books from Project Gutenberg packed into 10 ZIP archives."

gh release upload "$TAG" "$CORPUS_DIR"/*.zip --repo "$REPO"
