#!/usr/bin/env bash
# Post-build hook: fix wasm-bindgen-rayon's broken import path in the worker
# helper. Trunk renames the main JS file with a content hash, but the generated
# workerHelpers.no-bundler.js still contains import('../../..') which resolves
# to the dist directory root instead of the actual JS file.
set -e

DIST="dist"
# Trunk's main bundle (named after the crate, with a content hash); match dash or
# underscore spellings. Worker helpers live under $DIST/snippets, not here.
JS_FILE=$(ls "$DIST"/morpion*solitaire*wasm*.js 2>/dev/null | head -1)
if [ -z "$JS_FILE" ]; then
    echo "patch-worker.sh: no main bundle found in $DIST, skipping"
    exit 0
fi
JS_FILENAME=$(basename "$JS_FILE")

find "$DIST/snippets" -name "workerHelpers.no-bundler.js" | while read -r WORKER_FILE; do
    TMPFILE=$(mktemp)
    sed "s|import('../../..')|import('../../../$JS_FILENAME')|g" "$WORKER_FILE" > "$TMPFILE"
    mv "$TMPFILE" "$WORKER_FILE"
    echo "patch-worker.sh: patched $WORKER_FILE → $JS_FILENAME"
done
