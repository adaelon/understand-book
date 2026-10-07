#!/usr/bin/env python3
"""Run the ignored MU10 load against an isolated root and sample Linux /proc."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("test_binary", type=Path)
parser.add_argument("evidence_dir", type=Path)
args = parser.parse_args()
root = args.evidence_dir.resolve()
root.mkdir(parents=True, exist_ok=False)
temporary = root / "temporary"
temporary.mkdir()
memory = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
environment = {
    "cpus": os.cpu_count(),
    "memory_kib": int(memory["MemTotal"].split()[0]),
    "free_disk_bytes": shutil.disk_usage(root).free,
    "system": list(os.uname()),
}
(root / "environment.json").write_text(json.dumps(environment, indent=2))
env = dict(os.environ, TMPDIR=str(temporary), MU10_CAPACITY_REPORT=str(root / "capacity.json"))
with (root / "capacity.log").open("w") as output:
    process = subprocess.Popen(
        [str(args.test_binary.resolve()), "mu10_capacity_five_minutes", "--ignored", "--test-threads=1", "--nocapture"],
        env=env, stdout=output, stderr=output,
    )
    peak_rss = peak_threads = peak_fds = 0
    try:
        while process.poll() is None:
            try:
                status = dict(line.split(":", 1) for line in Path(f"/proc/{process.pid}/status").read_text().splitlines() if ":" in line)
                peak_rss = max(peak_rss, int(status.get("VmRSS", "0 kB").split()[0]))
                peak_threads = max(peak_threads, int(status.get("Threads", "0")))
                peak_fds = max(peak_fds, len(list(Path(f"/proc/{process.pid}/fd").iterdir())))
            except (FileNotFoundError, ProcessLookupError):
                pass  # Child exit can occur between the status and descriptor reads.
            time.sleep(0.25)
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
result = {"peak_rss_kib": peak_rss, "peak_threads": peak_threads, "peak_fds": peak_fds, "exit_code": process.returncode}
(root / "resources.json").write_text(json.dumps(result, indent=2))
print(json.dumps(result))
raise SystemExit(process.returncode or int(peak_rss > 4 * 1024 * 1024))
