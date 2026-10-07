"""Run the real launcher against an argument-recording executable in a release copy."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class LauncherTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mu11-launch-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        script = self.root / "scripts/linux/start-reader.sh"
        script.parent.mkdir(parents=True)
        shutil.copyfile(Path(__file__).with_name("start-reader.sh"), script)
        server = self.root / "target/release/server"
        server.parent.mkdir(parents=True)
        server.write_text('#!/usr/bin/env bash\nprintf "%s\\0" "$@"\n', encoding="utf-8")
        server.chmod(0o755)
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("UNDERSTAND_BOOK_")}
        self.env["MSYS_NO_PATHCONV"] = "1"
        self.bash = os.environ.get("MU11_BASH", "bash")

    def run_launcher(self, **env):
        return subprocess.run([self.bash, "scripts/linux/start-reader.sh"], cwd=self.root,
                              env={**self.env, **env}, capture_output=True, timeout=10)

    def args(self, result):
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        return result.stdout.decode().rstrip("\0").split("\0")

    def test_multi_user_uses_only_explicit_service_root_and_origin(self):
        result = self.run_launcher(UNDERSTAND_BOOK_MODE="multi-user",
                                   UNDERSTAND_BOOK_SERVICE_ROOT="/srv/readers with spaces",
                                   UNDERSTAND_BOOK_ORIGIN="https://reader.example")
        self.assertEqual(self.args(result), ["--multi-user", "/srv/readers with spaces", "--origin",
                                             "https://reader.example", "--addr", "127.0.0.1:8788"])

    def test_configured_backend_and_legacy_environment_do_not_change_mode(self):
        result = self.run_launcher(UNDERSTAND_BOOK_MODE="multi-user", UNDERSTAND_BOOK_DIR="/old/book",
                                   UNDERSTAND_BOOK_MEMORY_DIR="/old/memory", UNDERSTAND_BOOK_PRIVATE_DIR="/old/private",
                                   UNDERSTAND_BOOK_SERVICE_ROOT="/srv/new", UNDERSTAND_BOOK_ORIGIN="https://reader.example:9443",
                                   UNDERSTAND_BOOK_ADDR="127.0.0.1:9876")
        self.assertEqual(self.args(result), ["--multi-user", "/srv/new", "--origin",
                                             "https://reader.example:9443", "--addr", "127.0.0.1:9876"])

    def test_missing_multi_user_inputs_never_launch_server(self):
        for missing in ("UNDERSTAND_BOOK_SERVICE_ROOT", "UNDERSTAND_BOOK_ORIGIN"):
            with self.subTest(missing=missing):
                env = dict(UNDERSTAND_BOOK_MODE="multi-user", UNDERSTAND_BOOK_SERVICE_ROOT="/srv/new",
                           UNDERSTAND_BOOK_ORIGIN="https://reader.example")
                del env[missing]
                result = self.run_launcher(**env)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertIn(missing, result.stderr.decode())

    def test_unknown_mode_never_falls_back_to_single_reader(self):
        result = self.run_launcher(UNDERSTAND_BOOK_MODE="multi")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")

    def test_original_environment_keeps_single_reader_arguments(self):
        result = self.run_launcher(UNDERSTAND_BOOK_DIR="/books/my book", UNDERSTAND_BOOK_LIBRARY_ROOT="/books",
                                   UNDERSTAND_BOOK_MEMORY_DIR="/memory", UNDERSTAND_BOOK_PRIVATE_DIR="/private",
                                   UNDERSTAND_BOOK_NODE="/tools/node")
        self.assertEqual(self.args(result), ["--reader-only", "/books/my book"])


if __name__ == "__main__":
    unittest.main()
