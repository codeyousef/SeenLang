"""Exercise numeric-ID draft transport without network or tag-name lookups."""

from __future__ import annotations

import hashlib
import io
import json
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

from scripts import release_draft_api as draft


class DraftTransportTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.version = "0.22.7"
        self.blobs = {}
        self.assets = []
        for number, name in enumerate(draft.initial_names(self.version), 1):
            self.add_asset(number, name, name.encode())
        self.draft = True
        self.calls = []

    def add_asset(self, number: int, name: str, data: bytes) -> dict:
        asset = {
            "id": number, "name": name, "size": len(data),
            "digest": f"sha256:{hashlib.sha256(data).hexdigest()}",
            "state": "uploaded",
        }
        self.assets.append(asset)
        self.blobs[number] = data
        return asset

    def respond(self, method: str, url: str, *, data=None, accept=None):
        self.calls.append((method, url))
        if method == "GET" and url == f"{draft.API}/42":
            return io.BytesIO(json.dumps({
                "id": 42, "tag_name": f"v{self.version}", "draft": self.draft,
                "assets": self.assets,
            }).encode())
        if method == "GET" and url.startswith(f"{draft.API}/assets/"):
            return io.BytesIO(self.blobs[int(url.rsplit("/", 1)[1])])
        if method == "POST" and url.startswith(f"{draft.UPLOAD}/42/assets?name="):
            name = url.split("?name=", 1)[1]
            asset = self.add_asset(len(self.assets) + 1, name, data)
            return io.BytesIO(json.dumps(asset).encode())
        if method == "PATCH" and url == f"{draft.API}/42":
            self.draft = False
            return io.BytesIO(json.dumps({
                "id": 42, "tag_name": f"v{self.version}", "draft": False,
            }).encode())
        raise AssertionError(f"unexpected API route: {method} {url}")

    def run_mode(self, mode: str, *arguments: str) -> None:
        argv = ["release_draft_api.py", mode, "--version", self.version,
                "--release-id", "42", *arguments]
        with mock.patch.object(sys, "argv", argv), mock.patch.object(draft, "request", self.respond):
            draft.main()

    def test_probe_and_download_by_numeric_id(self) -> None:
        self.run_mode("probe")
        self.run_mode("download-inputs", "--output-dir", str(self.root))
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), sorted(draft.initial_names(self.version)))
        self.assertTrue(all("/tags/" not in url for _, url in self.calls))
        self.assertTrue(any(url == f"{draft.API}/assets/4" for _, url in self.calls))
        with self.assertRaises(draft.DraftError):
            self.run_mode("download-inputs", "--output-dir", str(self.root))

    def test_corrupt_download_is_rejected_without_leaving_file(self) -> None:
        indexed = self.assets[-1]
        with mock.patch.object(draft, "request", lambda *args, **kwargs: io.BytesIO(b"corrupt")):
            with self.assertRaises(draft.DraftError):
                draft.download(indexed, self.root / indexed["name"])
        self.assertFalse((self.root / indexed["name"]).exists())

    def test_upload_audit_and_publish_exact_set(self) -> None:
        upload = self.root / "seen-0.22.7-linux-x64.tar.gz"
        upload.write_bytes(b"linux archive")
        self.run_mode("upload", "--file", str(upload))
        names = draft.initial_names(self.version) + [upload.name]
        arguments = [item for name in names for item in ("--expected-name", name)]
        audit = self.root / "audit"
        audit.mkdir()
        self.run_mode("download-all", "--output-dir", str(audit), *arguments)
        self.assertEqual((audit / upload.name).read_bytes(), upload.read_bytes())
        self.run_mode("publish", *arguments, "--title", "Seen Language 0.22.7", "--notes", "Complete")
        self.assertFalse(self.draft)
        with self.assertRaises(draft.DraftError):
            self.run_mode("probe")

    def test_wrong_draft_identity_and_extra_asset_fail_closed(self) -> None:
        self.draft = False
        with self.assertRaises(draft.DraftError):
            self.run_mode("probe")
        self.draft = True
        self.add_asset(20, "foreign.bin", b"foreign")
        with self.assertRaises(draft.DraftError):
            self.run_mode("probe")

    def test_bad_redirect_does_not_forward_token(self) -> None:
        redirect = urllib.error.HTTPError(
            f"{draft.API}/assets/1", 302, "Found",
            {"Location": "https://evil.example/asset"}, None,
        )
        fake = mock.Mock()
        fake.open.side_effect = redirect
        with mock.patch.dict("os.environ", {"GH_TOKEN": "test-token"}), \
             mock.patch.object(draft.urllib.request, "build_opener", return_value=fake):
            with self.assertRaises(draft.DraftError):
                draft.request("GET", f"{draft.API}/assets/1", accept="application/octet-stream")
        redirect.close()
        self.assertEqual(fake.open.call_count, 1)


if __name__ == "__main__":
    unittest.main()
