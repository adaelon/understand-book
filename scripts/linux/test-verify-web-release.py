"""Regression coverage for the reader-only package that removed the admin app."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "verify_web_release", Path(__file__).with_name("verify-web-release.py"))
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


class WebReleaseTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        for entry, asset in (("home.html", "home.js"), ("index.html", "reader.js"),
                             ("admin/index.html", "admin.js")):
            path = self.root / entry
            path.parent.mkdir(exist_ok=True)
            (path.parent / "assets").mkdir(exist_ok=True)
            path.write_text(f'<script type="module" src="./assets/{asset}"></script>', encoding="utf-8")
            (path.parent / "assets" / asset).write_text("application script", encoding="utf-8")

    def test_complete_release(self):
        entries, assets = verify.verify_dist(self.root)
        self.assertEqual(len(entries), 3)
        self.assertIn("/admin/assets/admin.js", assets)

    def test_reader_only_package_is_rejected(self):
        (self.root / "admin/index.html").unlink()
        with self.assertRaisesRegex(ValueError, "Missing web entry: admin/index.html"):
            verify.verify_dist(self.root)

    def test_missing_admin_script_is_rejected(self):
        (self.root / "admin/assets/admin.js").unlink()
        with self.assertRaisesRegex(ValueError, "Missing web resource: /admin/assets/admin.js"):
            verify.verify_dist(self.root)

    def serve(self, url):
        route = url.removeprefix("https://example.test")
        entry = verify.ROUTES.get(route)
        path = self.root / (entry or route.lstrip("/"))
        return ("text/html" if entry else "text/javascript", path.read_bytes())

    def test_reader_fallback_on_admin_route_is_rejected_even_with_200(self):
        def wrong_page(url):
            if url.endswith("/admin/invites"):
                return "text/html", (self.root / "index.html").read_bytes()
            return self.serve(url)
        with patch.object(verify, "fetch", side_effect=wrong_page):
            with self.assertRaisesRegex(ValueError, "Wrong web entry served at /admin/invites"):
                verify.verify_published(self.root, "https://example.test")

    def test_stale_public_script_is_rejected(self):
        def stale(url):
            if url.endswith("/admin/assets/admin.js"):
                return "text/javascript", b"old application"
            return self.serve(url)
        with patch.object(verify, "fetch", side_effect=stale):
            with self.assertRaisesRegex(ValueError, "Published resource differs"):
                verify.verify_published(self.root, "https://example.test")

    def test_published_entry_points_and_assets(self):
        with patch.object(verify, "fetch", side_effect=self.serve):
            report = verify.verify_published(self.root, "https://example.test")
        self.assertIn("/admin/invites", report["routes"])
        self.assertEqual(report["static_files"], 3)


if __name__ == "__main__":
    unittest.main()
