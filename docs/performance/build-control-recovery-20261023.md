# Book build control recovery, 2026-10-23

The existing optimization-book invocation could not resume because persisted JSON control records contained only NUL bytes. The bounded diagnostic identified `readAutomaticBuildDispatch` as the failing read. The confirmed plan and invocation were retained.

Recovery quarantined the damaged records, retaining the original bytes:

- Two Root checkpoint/response files were reconstructed from invocation control projections and parent launch metadata.
- Six handoff projections and six dispatch projections were moved to a separate registry maintenance directory. The dispatch-projection reader requires every directory entry to have a valid projection filename, so adjacent renamed backups cannot remain there. An initial adjacent quarantine triggered that filename check on replay; relocating the twelve backups corrected it.
- Three runtime manifests, their three runtime handoffs, and three opaque registry records were isolated. Each runtime had no matching owned task claim, progress record, or terminal receipt. All six affected handoff refs had no executor open record; three intact opaque registry records were retained.

The recovery did not assume a one-to-one relationship between damaged projection and runtime counts. The six never-opened refs were removed from Root's local retired-ref set, allowing the Engine to issue them when appropriate. Accepted results and task attempts were preserved. Root checkpoint writes now use flush/fsync and atomic replacement; an injected replacement failure preserved the previous valid checkpoint.

The installed Engine then returned `SPAWN_EXECUTORS` for the same invocation: Pass1 468/468, Profile 6265/6265, BookStructure 6/1222 (accepted outline and five discovery fragments). Eight fresh semantic executors were started. This confirms control recovery and resumption, not completion of the remaining stages.

Bounded evidence is in `tmp/control-recovery-20261023/driver-diagnostic-summary.json` and `dispatch-repair-result.json`. The recovery script reads dispatch-control metadata and checks damaged files as bytes; it does not decode semantic inputs or candidates. The underlying cause of the NUL writes was not established.
