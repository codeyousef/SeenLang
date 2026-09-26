#!/usr/bin/env python3
"""Fail-closed GitHub draft transfer by numeric release and asset IDs.

Tag-name release lookup is deliberately absent: unpublished drafts are not
reliably visible to a workflow's generated token through that route.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

REPOSITORY = "codeyousef/SeenLang"
API = f"https://api.github.com/repos/{REPOSITORY}/releases"
UPLOAD = f"https://uploads.github.com/repos/{REPOSITORY}/releases"
DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*\Z")


class DraftError(ValueError):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


def token() -> str:
    value = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not value or "\n" in value or "\r" in value:
        raise DraftError("a GitHub workflow token is required")
    return value


def request(method: str, url: str, *, data: bytes | None = None,
            accept: str = "application/vnd.github+json"):
    headers = {
        "Authorization": f"Bearer {token()}",
        "Accept": accept,
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "SeenLang-release-draft",
    }
    if data is not None:
        headers["Content-Type"] = "application/octet-stream" if url.startswith(UPLOAD) else "application/json"
    call = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        return urllib.request.build_opener(NoRedirect()).open(call, timeout=60)
    except urllib.error.HTTPError as error:
        if method == "GET" and accept == "application/octet-stream" and error.code in (301, 302, 303, 307, 308):
            location = error.headers.get("Location", "")
            parsed = urllib.parse.urlsplit(location)
            host = (parsed.hostname or "").lower()
            if parsed.scheme != "https" or not (
                host == "githubusercontent.com" or host.endswith(".githubusercontent.com")
            ):
                raise DraftError("asset redirect target is not a trusted HTTPS GitHub host") from error
            # Signed asset URL needs no API bearer token. Never forward it to
            # another host through an automatic redirect.
            signed = urllib.request.Request(location, headers={"User-Agent": "SeenLang-release-draft"})
            return urllib.request.build_opener(NoRedirect()).open(signed, timeout=60)
        raise DraftError(f"GitHub {method} request failed (HTTP {error.code})") from error


def json_request(method: str, url: str, value: dict | None = None) -> dict:
    data = None if value is None else json.dumps(value).encode("utf-8")
    with request(method, url, data=data) as response:
        result = json.load(response)
    if not isinstance(result, dict):
        raise DraftError("GitHub returned a non-object response")
    return result


def release(release_id: int, version: str) -> dict[str, dict]:
    result = json_request("GET", f"{API}/{release_id}")
    if result.get("id") != release_id or result.get("tag_name") != f"v{version}" or result.get("draft") is not True:
        raise DraftError("numeric release ID is not the expected unpublished version draft")
    assets = result.get("assets")
    if not isinstance(assets, list):
        raise DraftError("draft asset list is missing")
    indexed = {}
    for asset in assets:
        if not isinstance(asset, dict):
            raise DraftError("invalid draft asset metadata")
        name = asset.get("name")
        if not isinstance(name, str) or not NAME.fullmatch(name) or name in indexed:
            raise DraftError("unsafe or duplicate draft asset name")
        if type(asset.get("id")) is not int or asset["id"] <= 0:
            raise DraftError("invalid draft asset ID")
        if type(asset.get("size")) is not int or asset["size"] <= 0:
            raise DraftError("invalid draft asset size")
        if not isinstance(asset.get("digest"), str) or not DIGEST.fullmatch(asset["digest"]):
            raise DraftError("draft asset lacks a SHA-256 digest")
        if asset.get("state") != "uploaded":
            raise DraftError("draft asset is not fully uploaded")
        indexed[name] = asset
    return indexed


def expected_assets(indexed: dict, expected: list[str]) -> None:
    if len(expected) != len(set(expected)) or set(indexed) != set(expected):
        raise DraftError("draft asset names differ from the exact expected set")


def download(asset: dict, output: Path | None) -> None:
    digest = hashlib.sha256()
    size = 0
    target = None
    if output is not None:
        if output.exists() or output.is_symlink():
            raise DraftError(f"refusing to overwrite {output.name}")
        target = output.open("xb")
    try:
        with request("GET", f"{API}/assets/{asset['id']}", accept="application/octet-stream") as response:
            while block := response.read(1024 * 1024):
                size += len(block)
                if size > asset["size"]:
                    raise DraftError("download exceeds declared asset size")
                digest.update(block)
                if target is not None:
                    target.write(block)
    except BaseException:
        if target is not None:
            target.close()
            output.unlink(missing_ok=True)
        raise
    else:
        if target is not None:
            target.close()
    if size != asset["size"] or f"sha256:{digest.hexdigest()}" != asset["digest"]:
        if output is not None:
            output.unlink(missing_ok=True)
        raise DraftError("downloaded asset does not match GitHub size and SHA-256")


def initial_names(version: str) -> list[str]:
    return [
        f"seen-{version}-macos-arm64.tar.gz",
        f"seen-{version}-windows-x64.zip",
        f"Seen-{version}-windows-x64-setup.exe",
        f"seen-{version}-platform-inputs.json",
    ]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("probe", "download-inputs", "assert-draft", "upload", "download-all", "publish"))
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-id", required=True, type=int)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--expected-name", action="append", default=[])
    parser.add_argument("--file", action="append", type=Path, default=[])
    parser.add_argument("--title")
    parser.add_argument("--notes")
    parser.add_argument("--prerelease", action="store_true")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", args.version) or args.release_id <= 0:
        raise DraftError("invalid version or numeric release ID")
    indexed = release(args.release_id, args.version)
    if args.mode in ("probe", "download-inputs", "upload"):
        expected_assets(indexed, initial_names(args.version))
    elif args.mode in ("assert-draft", "download-all", "publish"):
        expected_assets(indexed, args.expected_name)
    if args.mode == "probe":
        download(indexed[f"seen-{args.version}-platform-inputs.json"], None)
    elif args.mode in ("download-inputs", "download-all"):
        if args.output_dir is None or not args.output_dir.is_dir() or args.output_dir.is_symlink():
            raise DraftError("output directory must be an existing safe directory")
        for name, asset in indexed.items():
            download(asset, args.output_dir / name)
    elif args.mode == "upload":
        if not args.file:
            raise DraftError("no release artifacts supplied")
        names = [path.name for path in args.file]
        if len(names) != len(set(names)) or any(not NAME.fullmatch(name) for name in names):
            raise DraftError("unsafe or duplicate upload names")
        if set(names) & set(indexed):
            raise DraftError("upload would replace a staged asset")
        for path in args.file:
            if not path.is_file() or path.is_symlink() or path.stat().st_size == 0:
                raise DraftError(f"unsafe or missing upload: {path.name}")
        for path in args.file:
            data = path.read_bytes()
            query = urllib.parse.urlencode({"name": path.name})
            # Binary upload is intentionally not routed through tag-name CLI.
            with request("POST", f"{UPLOAD}/{args.release_id}/assets?{query}", data=data) as response:
                uploaded = json.load(response)
            if not isinstance(uploaded, dict):
                raise DraftError(f"GitHub returned invalid upload metadata: {path.name}")
            if uploaded.get("name") != path.name or uploaded.get("size") != len(data) or uploaded.get("digest") != f"sha256:{hashlib.sha256(data).hexdigest()}":
                raise DraftError(f"GitHub upload identity mismatch: {path.name}")
            indexed = release(args.release_id, args.version)
            expected_assets(indexed, initial_names(args.version) + names[:names.index(path.name) + 1])
    elif args.mode == "publish":
        if not args.title or not args.notes:
            raise DraftError("publication title and notes are required")
        result = json_request("PATCH", f"{API}/{args.release_id}", {
            "draft": False, "name": args.title, "body": args.notes,
            "prerelease": args.prerelease,
        })
        if result.get("id") != args.release_id or result.get("tag_name") != f"v{args.version}" or result.get("draft") is not False:
            raise DraftError("GitHub did not confirm publication of the exact release")
    print(f"PASS: numeric draft {args.release_id} {args.mode} v{args.version}")


if __name__ == "__main__":
    try:
        main()
    except (DraftError, OSError, urllib.error.URLError, json.JSONDecodeError, TypeError) as error:
        print(f"release-draft-api: {error}", file=sys.stderr)
        sys.exit(1)
