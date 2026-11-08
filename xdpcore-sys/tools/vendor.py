#!/usr/bin/env python3
"""Import committed upstream libxdp sources, or verify the pinned snapshot."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "vendor" / "xdp-tools"
MANIFEST = ROOT / "vendor" / "manifest.json"
REPOSITORY = "https://github.com/xdp-project/xdp-tools"
FILES = [
    "LICENSE", "version.mk",
    "headers/xdp/xsk.h", "headers/xdp/libxdp.h",
    "headers/xdp/xdp_helpers.h", "headers/xdp/prog_dispatcher.h",
    "lib/libxdp/libxdp.c", "lib/libxdp/xsk.c",
    "lib/libxdp/xdp-dispatcher.c.in", "lib/libxdp/xsk_def_xdp_prog.c",
    "lib/libxdp/xsk_def_xdp_prog_5.3.c", "lib/libxdp/libxdp_internal.h",
    "lib/libxdp/compat.h", "lib/libxdp/bpf_instr.h",
    "lib/libxdp/xsk_def_xdp_prog.h", "lib/libxdp/libxdp.map",
]


def git(source, *args):
    return subprocess.check_output(["git", "-C", str(source), *args])


def digest(contents):
    return hashlib.sha256(contents).hexdigest()


def verify():
    manifest = json.loads(MANIFEST.read_text())
    actual = {p.relative_to(VENDOR).as_posix() for p in VENDOR.rglob("*") if p.is_file()}
    expected = manifest["sha256"]
    if actual != set(expected):
        raise SystemExit(f"Vendor file list differs: {sorted(actual ^ set(expected))}")
    for name, checksum in expected.items():
        if digest((VENDOR / name).read_bytes()) != checksum:
            raise SystemExit(f"Upstream snapshot modified: {name}")
    print(f"Verified {len(expected)} upstream files at {manifest['revision']}")


def update(source, revision):
    revision = git(source, "rev-parse", "--verify", f"{revision}^{{commit}}").decode().strip()
    additional = git(source, "ls-tree", "-r", "--name-only", revision, "--", "headers/linux", "LICENSES").decode().splitlines()
    files = sorted(set(FILES + additional))
    contents = {name: git(source, "show", f"{revision}:{name}") for name in files}
    version = contents["version.mk"].decode().split('"')[1]
    old_files = json.loads(MANIFEST.read_text())["sha256"] if MANIFEST.exists() else {}
    for name in set(old_files) - set(files):
        path = (VENDOR / name).resolve()
        if not path.is_relative_to(VENDOR.resolve()):
            raise SystemExit(f"Invalid vendor manifest path: {name}")
        path.unlink()
    for name, data in contents.items():
        path = VENDOR / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    MANIFEST.write_text(json.dumps({
        "repository": REPOSITORY,
        "revision": revision,
        "version": version,
        "sha256": {name: digest(data) for name, data in contents.items()},
    }, indent=2) + "\n")
    verify()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true")
    action.add_argument("--source", type=Path)
    parser.add_argument("--revision", default="HEAD")
    args = parser.parse_args()
    if args.check:
        verify()
    else:
        update(args.source, args.revision)
