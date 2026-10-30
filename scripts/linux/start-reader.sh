#!/usr/bin/env bash
set -euo pipefail
source_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$source_root"
case "${UNDERSTAND_BOOK_MODE:-single-user}" in
  single-user)
    : "${UNDERSTAND_BOOK_DIR:?Set UNDERSTAND_BOOK_DIR to a complete book workspace}"
    : "${UNDERSTAND_BOOK_LIBRARY_ROOT:?Set UNDERSTAND_BOOK_LIBRARY_ROOT}"
    : "${UNDERSTAND_BOOK_MEMORY_DIR:?Set UNDERSTAND_BOOK_MEMORY_DIR}"
    : "${UNDERSTAND_BOOK_PRIVATE_DIR:?Set UNDERSTAND_BOOK_PRIVATE_DIR}"
    : "${UNDERSTAND_BOOK_NODE:?Set UNDERSTAND_BOOK_NODE to the Node executable}"
    export UNDERSTAND_BOOK_WEB_DIST="${UNDERSTAND_BOOK_WEB_DIST:-$source_root/packages/web/dist}"
    exec "$source_root/target/release/server" --reader-only "$UNDERSTAND_BOOK_DIR"
    ;;
  multi-user)
    : "${UNDERSTAND_BOOK_SERVICE_ROOT:?Set UNDERSTAND_BOOK_SERVICE_ROOT to the independent multi-user root}"
    : "${UNDERSTAND_BOOK_ORIGIN:?Set UNDERSTAND_BOOK_ORIGIN to the public HTTPS origin}"
    export UNDERSTAND_BOOK_EMBEDDING_CONFIG="${UNDERSTAND_BOOK_EMBEDDING_CONFIG:-/opt/understand-book/models/local-embedding.json}"
    if [[ ! -r "$UNDERSTAND_BOOK_EMBEDDING_CONFIG" ]]; then
      echo 'Semantic retrieval is not installed. Run scripts/linux/build-multi-reader.sh before publishing.' >&2
      exit 1
    fi
    export UNDERSTAND_BOOK_WEB_DIST="${UNDERSTAND_BOOK_WEB_DIST:-$source_root/packages/web/dist-multi}"
    exec "$source_root/target/release/server" --multi-user "$UNDERSTAND_BOOK_SERVICE_ROOT" \
      --origin "$UNDERSTAND_BOOK_ORIGIN" --addr "${UNDERSTAND_BOOK_ADDR:-127.0.0.1:8788}"
    ;;
  *)
    echo 'UNDERSTAND_BOOK_MODE must be single-user or multi-user' >&2
    exit 2
    ;;
esac
