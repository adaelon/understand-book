"""Check the three hosted entry points and their directly referenced static files."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from html.parser import HTMLParser
import json
from pathlib import Path
from urllib.parse import urljoin, urlsplit
from urllib.request import Request, urlopen


ROUTES = {
    "/": "home.html",
    "/index.html": "index.html",
    **{path: "admin/index.html" for path in (
        "/admin/", "/admin/login", "/admin/users", "/admin/invites",
        "/admin/usage", "/admin/monitor", "/admin/workspace-releases",
        "/admin/operator-builds",
    )},
}


class Resources(HTMLParser):
    def __init__(self, html, entry):
        super().__init__()
        self.base = "https://release.invalid/" + entry
        self.paths = set()
        self.scripts = set()
        self.feed(html)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        ref = attrs.get("src") if tag in ("script", "img") else None
        if tag == "link":
            ref = attrs.get("href")
        if not ref:
            return
        url = urlsplit(urljoin(self.base, ref))
        if url.netloc != "release.invalid":
            return
        self.paths.add(url.path)
        if tag == "script":
            self.scripts.add(url.path)


def verify_dist(root):
    root = Path(root)
    entries = {}
    assets = set()
    for entry in dict.fromkeys(ROUTES.values()):
        path = root / entry
        if not path.is_file():
            raise ValueError(f"Missing web entry: {entry}")
        parsed = Resources(path.read_text(encoding="utf-8"), entry)
        if not parsed.scripts:
            raise ValueError(f"Web entry has no application script: {entry}")
        for asset in sorted(parsed.paths):
            if not (root / asset.lstrip("/")).is_file():
                raise ValueError(f"Missing web resource: {asset} (from {entry})")
        entries[entry] = parsed.paths
        assets.update(parsed.paths)
    return entries, assets


def fetch(url):
    request = Request(url, headers={
        "User-Agent": "UnderstandBook-MCP10-Verification",
        "Cache-Control": "no-cache",
    })
    with urlopen(request, timeout=30) as response:
        return response.headers.get_content_type(), response.read()


def verify_published(root, origin):
    entries, assets = verify_dist(root)
    origin = origin.rstrip("/")
    # Independent static GETs: keep public verification bounded by four requests.
    with ThreadPoolExecutor(max_workers=4) as pool:
        responses = dict(pool.map(
            lambda path: (path, fetch(origin + path)), [*ROUTES, *sorted(assets)]))
    for route, entry in ROUTES.items():
        mime, body = responses[route]
        if mime != "text/html":
            raise ValueError(f"Wrong page content type: {route}: {mime}")
        actual = Resources(body.decode("utf-8"), entry)
        # Cloudflare can add its analytics script to HTML.
        if not entries[entry].issubset(actual.paths):
            raise ValueError(f"Wrong web entry served at {route}; expected {entry}")
    for asset in sorted(assets):
        mime, body = responses[asset]
        if body != (Path(root) / asset.lstrip("/")).read_bytes():
            raise ValueError(f"Published resource differs from release: {asset}")
        suffix = Path(asset).suffix
        if suffix in (".js", ".mjs") and mime not in (
            "application/javascript", "text/javascript",
        ):
            raise ValueError(f"Wrong JavaScript content type: {asset}: {mime}")
        if suffix == ".css" and mime != "text/css":
            raise ValueError(f"Wrong CSS content type: {asset}: {mime}")
    return {"routes": list(ROUTES), "static_files": len(assets)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dist", type=Path)
    parser.add_argument("--origin", help="Also verify the deployed HTTPS pages and files")
    args = parser.parse_args()
    entries, assets = verify_dist(args.dist)
    report = {"entries": list(entries), "static_files": len(assets)}
    if args.origin:
        report.update(verify_published(args.dist, args.origin))
    print(json.dumps({"status": "passed", **report}))


if __name__ == "__main__":
    main()
