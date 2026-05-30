#!/bin/bash
set -euo pipefail

REPO="ildar-safarov/gutenindex"
TAG="index-v1"

INDEX_DIR="${1:?usage: $0 <index_dir>}"

gh release create "$TAG" \
    --repo "$REPO" \
    --title "Gutenindex search index" \
    --notes "Binary inverted index for the Project Gutenberg books."

gh release upload "$TAG" \
    "$INDEX_DIR/vocab" \
    "$INDEX_DIR/0" \
    "$INDEX_DIR/1" \
    "$INDEX_DIR/2" \
    "$INDEX_DIR/3" \
    "$INDEX_DIR/4" \
    "$INDEX_DIR/5" \
    "$INDEX_DIR/6" \
    "$INDEX_DIR/7" \
    "$INDEX_DIR/doclen" \
    "$INDEX_DIR/meta" \
    --repo "$REPO"
