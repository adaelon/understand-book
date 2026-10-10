#!/usr/bin/env bash
set -euo pipefail
source_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$source_root"
# Build in the final release directory: runtime source paths are compiled in.
rm -f -- multi-reader-build.txt
pnpm install --frozen-lockfile
VITE_MULTI_USER=1 pnpm -C packages/web build --outDir dist-multi
pnpm -C apps/admin build
mkdir -p packages/web/dist-multi/admin
cp -a apps/admin/dist/. packages/web/dist-multi/admin/
cargo build --locked --release -p server --bin server --bin manage_reader \
  --bin publish_book --bin reader_maintenance --bin presentation_worker -j 1
# Keep the build receipt beside the binaries and Web files it describes.
# A working-tree build is labelled explicitly; HEAD alone is not its identity.
{
  printf 'built_at_utc=%s\nsource_root=%s\nprofile=release\nweb=packages/web/dist-multi\nadmin=packages/web/dist-multi/admin\n' "$(date -u +%FT%TZ)" "$source_root"
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    printf 'commit=%s\n' "$(git rev-parse HEAD)"
    if test -n "$(git status --porcelain --untracked-files=normal)"; then
      printf 'source_state=working-tree\n'
    else
      printf 'source_state=clean\n'
    fi
  else
    printf 'commit=unavailable\nsource_state=source-copy\n'
  fi
  rustc --version
  cargo --version
  node --version
  pnpm --version
} > multi-reader-build.txt
printf 'Multi-reader build complete: %s\n' "$source_root"
