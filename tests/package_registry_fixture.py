#!/usr/bin/env python3
"""Loopback-only HTTPS registry peer for the two native package acceptance tests.

This is not a registry deployment or a scanner. Publication remains quarantined;
activation is an explicit fixture operation using the normative TUF example's
synthetic review claims. All client archive, signature, and TLS checks stay on.
A local CONNECT endpoint maps canonical no-port test origins to an ephemeral
loopback listener, without DNS changes, privileged ports, or remote requests.
"""
from __future__ import annotations

import hashlib
import io
import json
import os
import secrets
import ssl
import subprocess
import sys
import re
import tarfile
import threading
import tomllib
from copy import deepcopy
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

from package_registry_contracts import CONTRACT, canonical_json_bytes as canonical

HOSTS = ("registry.fixture.test", "alternate.fixture.test")
REPOSITORY = "seen-dev-test-fixture-v1"
MAX_ARCHIVE = 25 * 1024 * 1024


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def openssl(*args):
    return subprocess.run(["openssl", *map(str, args)], check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=20).stdout


def save(path, value):
    path.write_bytes(canonical(value))


def load(path):
    return json.loads(path.read_bytes())


def stamp(delta=timedelta()):
    return (datetime.now(timezone.utc) + delta).strftime("%Y-%m-%dT%H:%M:%SZ")


def envelope(root, signed, roles):
    message = root / "signed.json"
    message.write_bytes(canonical(signed))
    signatures = []
    for role in roles:
        public = load(root / (role + ".public.json"))
        signature = openssl("pkeyutl", "-sign", "-rawin", "-inkey",
                            root / (role + ".pem"), "-in", message)
        signatures.append({"keyid": sha(canonical(public)), "sig": signature.hex()})
    return canonical({"signed": signed, "signatures": signatures})


def metadata(root, host, activate=False, malformed=False):
    template = load(CONTRACT / "fixtures/tuf-metadata-examples.json")["metadata"]
    directory = root / host
    generation = load(directory / "generation.json") + 1
    save(directory / "generation.json", generation)
    documents = {}
    names = {"targets": "targets", "releases": "release_targets",
             "security": "security_targets", "snapshot": "snapshot", "timestamp": "timestamp"}
    for role, name in names.items():
        signed = deepcopy(template[name]["signed"])
        signed.update(version=generation, expires=stamp(timedelta(days=1)))
        if "targets" in signed:
            signed["targets"] = {}
        documents[role] = signed
    keys = {role: load(root / (role + ".public.json"))
            for role in ("security", "releases")}
    delegations = documents["targets"]["delegations"]
    delegations["keys"] = {sha(canonical(key)): key for key in keys.values()}
    for role in delegations["roles"]:
        role["keyids"] = [sha(canonical(keys[role["name"]]))]
    if activate:
        release = load(directory / "release.json")
        assert release["state"]["lifecycle"] == "quarantined"
        digest = release["archive"]["sha256"]
        blob = (directory / "blob.tgz").read_bytes()
        assert sha(blob) == digest and len(blob) == release["archive"]["compressed_bytes"]
        target = deepcopy(next(iter(template["release_targets"]["signed"]["targets"].values())))
        owner, name = release["package"].split("/")
        custom = target["custom"]
        custom.update(registry_origin=f"https://{host}/packages", package=release["package"],
                      owner=owner, name=name, version=release["version"], archive_sha256=digest,
                      archive_filename=f"{name}-{release['version']}.seenpkg.tgz",
                      blob={"sha256": digest, "length": len(blob)}, dependencies=[],
                      capabilities=[], activated_at=stamp(timedelta(minutes=-1)))
        subject = {key: custom[key] for key in ("package", "owner", "name", "version", "blob", "visibility")}
        projection = {key: custom[key] for key in ("publisher_principal", "registry_service_identity",
                       "source_repository", "source_commit", "review", "activated_at")}
        projection["subject"] = subject
        attestation = sha(canonical(projection))
        custom.update(registry_attestation_sha256=attestation, provenance_sha256=attestation)
        target.update(length=len(blob), hashes={"sha256": digest})
        path = f"packages/{owner}/{name}/{release['version']}/{digest}/{custom['archive_filename']}"
        if malformed:
            custom["archive_sha256"] = "../../victim"
        documents["releases"]["targets"][path] = target
    raw = {}
    def binding(role):
        return {"version": generation, "length": len(raw[role]), "hashes": {"sha256": sha(raw[role])}}
    for role in ("targets", "releases", "security", "snapshot", "timestamp"):
        if role == "snapshot":
            documents[role]["meta"] = {name + ".json": binding(name) for name in ("targets", "releases", "security")}
        if role == "timestamp":
            documents[role]["meta"] = {"snapshot.json": binding("snapshot")}
        raw[role] = envelope(root, documents[role], [role])
        filename = "timestamp.json" if role == "timestamp" else f"{generation}.{role}.json"
        (directory / filename).write_bytes(raw[role])


def release_record(identity, request):
    return {"package": identity, "version": request["version"], "archive": request["archive"],
            "manifest_sha256": request["manifest_sha256"], "capabilities": [],
            "state": {"lifecycle": "reserved", "visibility": "public",
                      "availability": "unavailable", "retention": "retained"}}


class Registry(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def reply(self, status, body=b"", headers=None):
        if isinstance(body, dict):
            body = canonical(body)
        self.send_response(status)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Content-Type", "application/json")
        for name, value in (headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)

    def do_CONNECT(self):
        if self.path not in [host + ":443" for host in HOSTS]:
            self.reply(403)
            return
        self.send_response(200)
        self.end_headers()
        self.wfile.flush()
        connection = self.server.tls.wrap_socket(self.connection, server_side=True)
        try:
            Registry(connection, self.client_address, self.server)
        finally:
            connection.close()
        self.close_connection = True

    def route(self, method):
        host = self.headers.get("Host")
        if host not in HOSTS or not isinstance(self.connection, ssl.SSLSocket):
            self.reply(403)
            return
        directory = self.server.root / host
        prefix = "/packages/api/v1/"
        if not self.path.startswith(prefix):
            self.reply(404)
            return
        path = self.path[len(prefix):]
        with (self.server.root / "requests.log").open("a") as log:
            log.write(f"{host} {method} {path}\n")
        if method == "GET":
            if path.startswith("metadata/"):
                filename = path.removeprefix("metadata/")
                if "/" in filename or filename.startswith("."):
                    self.reply(404)
                    return
                file = directory / filename
                self.reply(200, file.read_bytes()) if file.is_file() else self.reply(404)
            elif path.startswith("blobs/sha256/") and (directory / "active").exists():
                release = load(directory / "release.json")
                if path == "blobs/sha256/" + release["archive"]["sha256"]:
                    self.reply(200, (directory / "blob.tgz").read_bytes())
                else:
                    self.reply(404)
            else:
                self.reply(404)
            return
        token = (self.server.root / "token").read_text()
        if self.headers.get("Authorization") != "Bearer " + token:
            self.reply(401, {"error": {"code": "unauthorized", "message": "fixture credential required"}})
            return
        length = int(self.headers.get("Content-Length", "0"))
        if not 0 < length <= MAX_ARCHIVE:
            self.reply(413)
            return
        body = self.rfile.read(length)
        if len(body) != length:
            self.reply(400)
            return
        if method == "POST" and not self.headers.get("Idempotency-Key"):
            self.reply(400)
            return
        if method == "PUT" and path == "uploads/upl_0123456789abcdef/archive":
            release = load(directory / "release.json")
            digest = sha(body)
            if (digest != release["archive"]["sha256"] or length != release["archive"]["compressed_bytes"]
                    or self.headers.get("X-Seen-Archive-Sha256") != digest
                    or self.headers.get("Content-Type") != "application/gzip"):
                self.reply(400)
                return
            with tarfile.open(fileobj=io.BytesIO(body), mode="r:gz") as archive:
                member = archive.extractfile("Seen.toml")
                assert member is not None
                manifest = member.read()
            assert sha(manifest) == release["manifest_sha256"]
            parsed = tomllib.loads(manifest.decode())
            assert parsed["package"]["identity"] == release["package"]
            assert parsed["project"]["version"] == release["version"]
            (directory / "blob.tgz").write_bytes(body)
            confirmation = "0" * 64 if (directory / "bad-confirmation").exists() else digest
            self.reply(204, headers={"X-Seen-Archive-Sha256": confirmation, "ETag": f'"sha256:{confirmation}"'})
            return
        request = json.loads(body)
        if method == "POST" and path == "packages":
            save(directory / "identity.json", request["identity"])
            self.reply(201, {"identity": request["identity"]})
        elif method == "POST" and path == "packages/" + load(directory / "identity.json") + "/releases":
            assert request["source"]["repository_id"] == "987654321"
            release = release_record(load(directory / "identity.json"), request)
            save(directory / "release.json", release)
            self.reply(201, {"release": release, "upload": {
                "upload_id": "upl_0123456789abcdef", "method": "PUT",
                "path": "/packages/api/v1/uploads/upl_0123456789abcdef/archive",
                "expires_at": stamp(timedelta(hours=1)), "maximum_bytes": MAX_ARCHIVE,
                "required_headers": {"Content-Type": "application/gzip", "Content-Length": release["archive"]["compressed_bytes"],
                                     "X-Seen-Archive-Sha256": release["archive"]["sha256"]}}})
        elif method == "POST" and path == "uploads/upl_0123456789abcdef/complete":
            release = load(directory / "release.json")
            assert request == {"archive_sha256": release["archive"]["sha256"], "compressed_bytes": release["archive"]["compressed_bytes"]}
            assert sha((directory / "blob.tgz").read_bytes()) == request["archive_sha256"]
            release["state"]["lifecycle"] = "quarantined"
            save(directory / "release.json", release)
            self.reply(202, release)
        else:
            self.reply(404)

    def do_GET(self):
        self.route("GET")

    def do_POST(self):
        self.route("POST")

    def do_PUT(self):
        self.route("PUT")


def run(root, command):
    root.mkdir(exist_ok=True)
    os.chmod(root, 0o700)
    openssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
            "-subj", "/CN=registry.fixture.test", "-addext", "subjectAltName=" + ",".join("DNS:" + host for host in HOSTS),
            "-keyout", root / "tls.key", "-out", root / "tls.crt")
    roles = ("root", "targets", "snapshot", "timestamp", "releases", "security")
    keys = {}
    for role in roles:
        private = root / (role + ".pem")
        openssl("genpkey", "-algorithm", "ED25519", "-out", private)
        public = openssl("pkey", "-in", private, "-pubout", "-outform", "DER")
        assert public.startswith(bytes.fromhex("302a300506032b6570032100")) and len(public) == 44
        keys[role] = {"keytype": "ed25519", "scheme": "ed25519", "keyval": {"public": public[-32:].hex()}}
        save(root / (role + ".public.json"), keys[role])
    signed = {"_type": "root", "spec_version": "1.0", "version": 1,
              "expires": stamp(timedelta(days=2)), "environment": "development", "repository_id": REPOSITORY,
              "consistent_snapshot": True, "keys": {sha(canonical(key)): key for key in keys.values()},
              "roles": {role: {"keyids": [sha(canonical(keys[role]))], "threshold": 1}
                        for role in ("root", "targets", "snapshot", "timestamp")}}
    root_bytes = envelope(root, signed, ["root"])
    (root / "root.json").write_bytes(root_bytes)
    token = secrets.token_hex(32)
    (root / "token").write_text(token)
    os.chmod(root / "token", 0o600)
    for host in HOSTS:
        (root / host).mkdir()
        save(root / host / "generation.json", 0)
        metadata(root, host)
    with HTTPServer(("127.0.0.1", 0), Registry) as server:
        server.root = root
        server.timeout = 1
        server.tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server.tls.minimum_version = ssl.TLSVersion.TLSv1_2
        server.tls.load_cert_chain(root / "tls.crt", root / "tls.key")
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        environment = dict(os.environ)
        environment.pop("SEEN_REGISTRY_TOKEN", None)
        environment.update(SEEN_REGISTRY_FIXTURE_ROOT=str(root), SSL_CERT_FILE=str(root / "tls.crt"),
                           HTTPS_PROXY=f"http://127.0.0.1:{server.server_port}", NO_PROXY="", no_proxy="",
                           SEEN_PACKAGE_CACHE=str(root / "cache"), SEEN_REGISTRY_TOKEN_FILE=str(root / "token"),
                           SEEN_REGISTRY_ORIGIN=f"https://{HOSTS[0]}/packages", SEEN_REGISTRY_ROOT_SHA256=sha(root_bytes))
        try:
            return subprocess.call(command, env=environment)
        finally:
            server.shutdown()
            thread.join()


def main():
    action, directory, *arguments = sys.argv[1:]
    root = Path(directory).resolve()
    if action == "restyle-lock":
        lock = root / "Seen.lock"
        original = lock.read_text()
        styled = re.sub(r'(?m)^([a-z_]+) = "([^"]*)"$',
                        r"\1 = '\2' # equivalent literal-string spelling", original)
        styled = styled.replace("[[packages]]", "[[ packages ]]")
        styled = styled.replace("[[root.dependencies]]", "[[ root.dependencies ]]")
        assert tomllib.loads(styled) == tomllib.loads(original)
        lock.write_text(styled)
        return 0
    if action == "run":
        return run(root, arguments)
    host = arguments[0] if arguments else HOSTS[0]
    assert host in HOSTS
    if action in ("activate", "malformed-digest"):
        metadata(root, host, activate=True, malformed=action == "malformed-digest")
        (root / host / "active").touch()
    elif action == "tamper-blob":
        blob = root / host / "blob.tgz"
        raw = blob.read_bytes()
        blob.write_bytes(bytes([raw[0] ^ 1]) + raw[1:])
    else:
        raise ValueError("unsupported fixture operation")
    return 0


if __name__ == "__main__":
    sys.exit(main())
