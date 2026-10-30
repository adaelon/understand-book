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
        shutil.copyfile(Path(__file__).with_name("verify-web-release.py"), script.with_name("verify-web-release.py"))
        local = self.root / "packages/web/dist"
        local.mkdir(parents=True)
        (local / "index.html").write_text("local reader")
        tools = self.root / "tools"
        tools.mkdir()
        scripts = {
            "pnpm": '''printf '%s\\n' "$*" >> commands.log
if [ "${1:-}" = --version ]; then echo test-pnpm; fi
if [ "${2:-}" = packages/web ] && [ "${3:-}" = build:multi ]; then
  mkdir -p packages/web/dist-multi/assets
  echo '<script src="/assets/reader.js"></script>' > packages/web/dist-multi/index.html
  cp packages/web/dist-multi/index.html packages/web/dist-multi/home.html
  echo script > packages/web/dist-multi/assets/reader.js
fi
if [ "${2:-}" = apps/admin ] && [ "${3:-}" = build ]; then
  if [ "${FAIL_ADMIN:-}" = 1 ]; then exit 1; fi
  mkdir -p apps/admin/dist/assets
  echo '<script src="/admin/assets/admin.js"></script>' > apps/admin/dist/index.html
  echo script > apps/admin/dist/assets/admin.js
  echo MIT > apps/admin/dist/LICENSE.shadcn-admin.txt
fi
''',
            "cargo": '''printf '%s\\n' "$*" >> commands.log
if [ "${1:-}" = build ] && [ "${FAIL_CARGO:-}" = 1 ]; then exit 1; fi
echo test-cargo
''',
            "rustc": "echo test-rustc\n",
            "node": '''printf '%s\\n' "$*" >> commands.log
if [ "${1:-}" = --import ] && [ "${FAIL_EMBEDDING:-}" = 1 ]; then exit 1; fi
echo test-node
''',
            "git": "exit 1\n",
        }
        for name, body in scripts.items():
            p = tools / name
            p.write_bytes(("#!/usr/bin/env bash\nset -eu\n" + body).encode("utf-8"))
            p.chmod(0o755)
        self.env = {**os.environ, "PATH": str(tools) + os.pathsep + os.environ["PATH"]}
        self.bash = os.environ.get("MU11_BASH", "bash")

    def run_build(self, fail=False, fail_admin=False, fail_embedding=False):
        return subprocess.run([self.bash, "scripts/linux/build-multi-reader.sh"], cwd=self.root,
                              env={**self.env, "FAIL_CARGO": "1" if fail else "0", "FAIL_ADMIN": "1" if fail_admin else "0", "FAIL_EMBEDDING": "1" if fail_embedding else "0"},
                              capture_output=True, text=True, timeout=20)

    def test_network_build_preserves_local_dist_and_records_source_copy(self):
        result = self.run_build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "packages/web/dist/index.html").read_text(), "local reader")
        self.assertIn('/assets/reader.js', (self.root / "packages/web/dist-multi/index.html").read_text())
        receipt = (self.root / "multi-reader-build.txt").read_text()
        self.assertIn("profile=release", receipt)
        self.assertIn("source_state=source-copy", receipt)
        self.assertIn("admin=packages/web/dist-multi/admin", receipt)
        self.assertIn('/admin/assets/admin.js', (self.root / "packages/web/dist-multi/admin/index.html").read_text())
        self.assertTrue((self.root / "packages/web/dist-multi/admin/assets/admin.js").exists())
        self.assertTrue((self.root / "packages/web/dist-multi/admin/LICENSE.shadcn-admin.txt").exists())
        commands = (self.root / "commands.log").read_text()
        self.assertLess(commands.index("packages/web build:multi"), commands.index("apps/admin build"))
        self.assertLess(commands.index("scripts/prepare-local-embedding.mjs"), commands.index("packages/web build:multi"))
        for name in ["server", "manage_reader", "publish_book", "reader_maintenance", "presentation_worker"]:
            self.assertIn("--bin " + name, commands)

    def test_failed_rebuild_does_not_leave_previous_success_receipt(self):
        (self.root / "multi-reader-build.txt").write_text("old success")
        result = self.run_build(fail=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "multi-reader-build.txt").exists())
        self.assertNotIn("Multi-reader build complete", result.stdout)

    def test_admin_failure_does_not_emit_a_success_receipt(self):
        (self.root / "multi-reader-build.txt").write_text("old success")
        result = self.run_build(fail_admin=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "multi-reader-build.txt").exists())

    def test_model_installation_failure_does_not_emit_a_success_receipt(self):
        (self.root / "multi-reader-build.txt").write_text("old success")
        result = self.run_build(fail_embedding=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "multi-reader-build.txt").exists())
        self.assertNotIn("packages/web build:multi", (self.root / "commands.log").read_text())


if __name__ == "__main__":
    unittest.main()
