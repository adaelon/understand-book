# EX11.1–EX11.2 implementation evidence

- `scene.html`, `ex11.1/`: engineering point/line scene written through production Author with `libraries:["konva"]`; three previews, delivery, Reader mouse/touch drag, saved scene, reopen and follow-up.
- `input.json`, `frozen.json`: diagnostic learning-rate input and run conditions.
- `frozen-source/`, `frozen-v2/` through `frozen-v5/`: method and implementation snapshots for the corresponding diagnostic batches.
- `ex11.2-run1/`: ex11.v1 received by the actual model; no write, only a premature construction promise. Retained failure.
- `ex11.2-run2/`: ex11.v2 draft; delivery emitted as text, repeated readouts before graphic, static-boundary and restoration failures.
- `ex11.2-run3/`: ex11.v3 delivered page, 126 independent numerical/geometry cases and real hit graph/pointer checks. Its `revision/` failed with Provider length; `revision-compact/` delivered continuous time; `revision-hitarea/` delivered boundary hits but failed Reader resize; `revision-resize/` was blocked by HTTP 402.
- `repair-*.txt`: exact feedback for each revision.
- `ex11.2-run3/engineering-resize-repair/`: separately labelled local resize correction, not a model-delivered version. Numeric/geometry and real pointer tests pass; desktop/narrow Reader pass; short Reader host overlay remains EX11.5.
- `density/`: original before/after at η=.8, k=2, p=0 in the same three viewports.
- `audit.py`: derive raw model HTML, method-load evidence, source-exclusion check and Provider token totals from recorded requests/responses.
- `frozen-resume1/`, `ex11.2-run3/revision-resize-resume1/`: final actual model retry after the user restored balance. Delivered revision 3, 252 math/geometry cases, six pointer drags and desktop/narrow Reader pass. 36 observed source chunks are contiguous. Short Reader native slider drag still fails; exact cause remains EX11.5. `verdict.json` and `source-diff.txt` record the result and model scope deviation.

Engineering verification (repository root):

```powershell
cargo test -p runtime --lib presentation_ -- --test-threads=1
cargo test -p server --lib presentation_author_tests -- --test-threads=1
cargo test -p server --lib ex11_preflight -- --ignored --nocapture
$env:EX10_CONTENT=(Resolve-Path docs/performance/ex11-local-demonstrations/ex11.1/content.json).Path
cargo test -p server --lib presentation_ex10_browser_host -- --ignored --nocapture
```

While the saved-content host is running, from `packages/web`:

```powershell
pnpm exec playwright test playwright/agent-presentation-ex11.spec.ts --config playwright.ex10.config.ts --workers=1 --reporter=line
```

The host name is the existing content-fixture entrypoint; supply the EX11 content file. Stop its bounded session with POST `http://127.0.0.1:4175/stop` before relinking Windows server tests. The Reader path uses the production component, bridge and storage APIs.

For a new, explicitly chosen diagnostic directory, set `EX11_RUN_DIR` and run `cargo test -p server --lib ex11_agent_diagnostic -- --ignored --nocapture`. The test refuses an existing directory. The adapter records production requests without changing instructions or tool arguments. Configuration is taken from the existing provider environment; credentials are not copied to evidence.

The runtime test fixture uses a shared name with millisecond filenames; run Author tests with `--test-threads=1` to avoid fixture collisions. The first parallel regression attempt collided before Author execution; the serial regression passed.

Final local checks (repository root; no model calls):

```powershell
cargo test -p runtime --lib tool_result::tests
node docs/performance/ex11-local-demonstrations/check-continuous.mjs docs/performance/ex11-local-demonstrations/ex11.2-run3/engineering-resize-repair
node docs/performance/ex11-local-demonstrations/check-pointer.mjs docs/performance/ex11-local-demonstrations/ex11.2-run3/engineering-resize-repair
```

To reproduce Reader, point the existing host's `EX10_CONTENT` to `engineering-resize-repair/content.json`, then set `EX11_RUN_DIR` to that directory and run `playwright/agent-presentation-ex11-generated.spec.ts` with `playwright.ex10.config.ts`. `EX11_WIDTH=960` or `320` selects a passing viewport; `640` reproduces the known host overlay. The full three-view test remains red for EX11.5; its first two results are retained in `reader-check.json`.

The user restored Provider balance and the following actual run completed. For any future reproduction choose a **new** revision name; this recorded directory already exists:

```powershell
$env:EX11_BASE_DIR=(Resolve-Path docs/performance/ex11-local-demonstrations/ex11.2-run3).Path
$env:EX11_REFERENCE_DIR=(Resolve-Path docs/performance/ex11-local-demonstrations/ex11.2-run3/revision-hitarea).Path
$env:EX11_REVISION_NAME='revision-resize-resume1'
$env:EX11_REVISION_INPUT=(Resolve-Path docs/performance/ex11-local-demonstrations/repair-resize-input.txt).Path
cargo test -p server --lib ex11_agent_revision -- --ignored --nocapture
```

The retry used frozen-resume1: v5 method, contiguous source projection and Windows CRLF frontmatter support. Actual requests and all three previews are retained. Final independent checks use the new artifact selectors and actual axis ticks, because its clip extends beyond the axes. The model changed more than resize; its unchanged-logic claim is not accepted as evidence. Reader dragHit records the parent element at the touch start; the short-screen failure starts on IFRAME, so its cause must be investigated rather than assumed to match the old overlay.


## EX11.3–EX11.5

- `ex11.3/`: engineering Manim scene, actual MP4, four decoded frames, metadata, cancellation evidence and saved content.
- `ex11.4/`: engineering two-demo page, formal preview/deliver, decoded pixel checks, saved/reopened/follow-up state and media lifecycle. `restore-failure.json` retains the earlier frame-callback restoration failure.
- `ex11.5/before`, `ex11.5/after`: original model page mounted unchanged; measured 1px iframe/touch failure and fixed 160px iframe with mouse/touch, last controls and question. The original EX11.2 records remain unchanged.

From the repository root:

```powershell
$env:UNDERSTAND_BOOK_ANIMATION_PYTHON=(Resolve-Path tmp/ex9a-manim-venv/Scripts/python.exe).Path
cargo test -p server --lib ex11_animation -- --ignored --test-threads=1 --nocapture
cargo test -p server --lib ex11_media_formal_preview_delivery -- --ignored --test-threads=1 --nocapture
$env:EX10_CONTENT=(Resolve-Path docs/performance/ex11-local-demonstrations/ex11.4/content.json).Path
cargo test -p server --lib presentation_ex10_browser_host -- --ignored --nocapture
```

While that bounded host runs, use `pnpm exec playwright test playwright/agent-presentation-media.spec.ts --config playwright.ex10.config.ts --workers=1` from `packages/web`. Stop the fixture with POST `http://127.0.0.1:4175/stop` before relinking Rust tests. For the short-screen regression select the original `ex11.2-run3/revision-resize-resume1/content.json` as `EX10_CONTENT`, then run `agent-presentation-short.spec.ts`. The bridge test is standalone; run `presentation-bridge.spec.ts` through the same Playwright config. Full original-page regression uses `EX11_RUN_DIR` pointing to `ex11.5/after` and `agent-presentation-ex11-generated.spec.ts`.

## EX11.6–EX11.7

See `../explorable-explanation-ex11-6-7-20260928.md` for the original C1 completion diagnosis and each transfer batch's verdict. `ex11.7/batch*/frozen.json` records natural inputs, model alias, method and code snapshot; `cost-summary.json` separates completed, failed, running and unstarted runs. Missing usage is unknown, not zero. No generated page is manually repaired for acceptance.

Create a new batch with `python docs/performance/ex11-local-demonstrations/freeze-ex11-7.py <new-batch-name>` from the repository root. Set `EX11_INPUT_FILE` to one frozen `inputs/*.json`, `EX11_RUN_DIR` to its new `runs/<id>` path, and use the existing `ex11_agent_diagnostic` test with the configured Provider and Manim Python. Do not reuse a populated run directory. After the request has ended, run `audit.py <run-path>` and `summarize-ex11-7.py <batch-path>`.

For a delivered page, adapt independent numeric/geometry observations to its untouched source. `check-ex11-7-canvas.mjs` is specific to batch1; `check-ex11-7-svg.mjs` is specific to batch3. They are not universal graders. Start the bounded Reader host with `EX10_CONTENT=<run>/content.json`; set `EX11_RUN_DIR` and add `reader-selectors.json` describing the actual `parameter`, `play`, `previous` and `result` selectors. Run `agent-presentation-ex11-transfer.spec.ts` through `playwright.ex10.config.ts`. The test uses real host save, reopen and follow-up requests across three viewports; it retains failed records. Stop the host before rebuilding Rust tests.
