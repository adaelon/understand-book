#!/usr/bin/env bash
# Source before starting the server, with its environment and service identity.
preview_browser="${UNDERSTAND_BOOK_PREVIEW_BROWSER:-}"
if [[ -z "$preview_browser" ]]; then
  for candidate in /usr/bin/chromium /usr/bin/chromium-browser /usr/bin/google-chrome; do
    if [[ -f "$candidate" ]]; then preview_browser="$candidate"; break; fi
  done
fi
if [[ -z "$preview_browser" || ! -f "$preview_browser" || ! -x "$preview_browser" ]]; then
  echo 'BOARD_PREVIEW_UNAVAILABLE: Set UNDERSTAND_BOOK_PREVIEW_BROWSER to an executable Chrome/Chromium path.' >&2
  exit 1
fi
for resource in renderer.js renderer.css; do
  if [[ ! -r "$UNDERSTAND_BOOK_WEB_DIST/board-preview/$resource" ]]; then
    echo "BOARD_PREVIEW_UNAVAILABLE: Cannot read board-preview/$resource in $UNDERSTAND_BOOK_WEB_DIST" >&2
    exit 1
  fi
done
"$preview_browser" --version >/dev/null
export UNDERSTAND_BOOK_PREVIEW_BROWSER="$preview_browser"
