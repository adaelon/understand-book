"""Verify release assembly and failed-build receipts without compiling dependencies."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class BuildTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mu11 build ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        script = self.root / "scripts/linux/build-multi-reader.sh"
        script.parent.mkdir(parents=True)
        shutil.copyfile(Path(__file__).with_name("build-multi-reader.sh"), script)
        local = self.root / "packages/web/dist"
        local.mkdir(parents=True)
        (local / "index.html").write_text("local reader")
        tools = self.root / "tools"
        tools.mkdir()
        scripts = {
            "pnpm": '''printf '%s\\n' "$*" >> commands.log
if [ "${1:-}" = --version ]; then echo test-pnpm; fi
if [ "${3:-}" = build ]; then mkdir -p "packages/web/${5}"; echo "$VITE_MULTI_USER" > "packages/web/${5}/index.html"; fi
''',
            "cargo": '''printf '%s\\n' "$*" >> commands.log
if [ "${1:-}" = build ] && [ "${FAIL_CARGO:-}" = 1 ]; then exit 1; fi
echo test-cargo
''',
            "rustc": "echo test-rustc\n",
            "node": "echo test-node\n",
            "git": "exit 1\n",
        }
        for name, body in scripts.items():
            p = tools / name
            p.write_text("#!/usr/bin/env bash\nset -eu\n" + body, encoding="utf-8", newline="\n")
            p.chmod(0o755)
        self.env = {**os.environ, "PATH": str(tools) + os.pathsep + os.environ["PATH"]}
        self.bash = os.environ.get("MU11_BASH", "bash")

    def run_build(self, fail=False):
        return subprocess.run([self.bash, "scripts/linux/build-multi-reader.sh"], cwd=self.root,
                              env={**self.env, "FAIL_CARGO": "1" if fail else "0"},
                              capture_output=True, text=True, timeout=20)

    def test_network_build_preserves_local_dist_and_records_source_copy(self):
        result = self.run_build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "packages/web/dist/index.html").read_text(), "local reader")
        self.assertEqual((self.root / "packages/web/dist-multi/index.html").read_text().strip(), "1")
        receipt = (self.root / "multi-reader-build.txt").read_text()
        self.assertIn("profile=release", receipt)
        self.assertIn("source_state=source-copy", receipt)
        commands = (self.root / "commands.log").read_text()
        for name in ["server", "manage_reader", "publish_book", "reader_maintenance", "presentation_worker"]:
            self.assertIn("--bin " + name, commands)

    def test_failed_rebuild_does_not_leave_previous_success_receipt(self):
        (self.root / "multi-reader-build.txt").write_text("old success")
        result = self.run_build(fail=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "multi-reader-build.txt").exists())
        self.assertNotIn("Multi-reader build complete", result.stdout)


if __name__ == "__main__":
    unittest.main()
