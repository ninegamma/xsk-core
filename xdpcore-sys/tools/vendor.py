#!/usr/bin/env python3
"""Import upstream libxdp sources, optionally regenerate bindings, or verify the snapshot."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "vendor" / "xdp-tools"
MANIFEST = ROOT / "vendor" / "manifest.json"
BUILD_RS = ROOT / "build.rs"
REPOSITORY = "https://github.com/xdp-project/xdp-tools"
BUILD_VERSION_PATTERN = re.compile(r'(?m)^const LIBXDP_VERSION: &str = "([^"]+)";$')
BPF_INCLUDE_PATTERN = re.compile(
    r"^cargo:rustc-env=XDPCORE_SYS_BPF_INCLUDE=(.+)$", re.MULTILINE
)
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


def build_rs_version():
    versions = BUILD_VERSION_PATTERN.findall(BUILD_RS.read_text())
    if len(versions) != 1:
        raise SystemExit(f"Expected one LIBXDP_VERSION declaration in {BUILD_RS}")
    return versions[0]


def set_build_rs_version(version):
    contents = BUILD_RS.read_text()
    updated, count = BUILD_VERSION_PATTERN.subn(
        f'const LIBXDP_VERSION: &str = "{version}";', contents
    )
    if count != 1:
        raise SystemExit(f"Expected one LIBXDP_VERSION declaration in {BUILD_RS}")
    BUILD_RS.write_text(updated)


def verify():
    manifest = json.loads(MANIFEST.read_text())
    current_version = build_rs_version()
    if current_version != manifest["version"]:
        raise SystemExit(
            f"build.rs LIBXDP_VERSION is {current_version}, "
            f"but vendored libxdp is {manifest['version']}"
        )
    actual = {p.relative_to(VENDOR).as_posix() for p in VENDOR.rglob("*") if p.is_file()}
    expected = manifest["sha256"]
    if actual != set(expected):
        raise SystemExit(f"Vendor file list differs: {sorted(actual ^ set(expected))}")
    for name, checksum in expected.items():
        if digest((VENDOR / name).read_bytes()) != checksum:
            raise SystemExit(f"Upstream snapshot modified: {name}")
    print(f"Verified {len(expected)} upstream files at {manifest['revision']}")


def cargo_target_dir():
    metadata = subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT.parent,
        text=True,
    )
    return Path(json.loads(metadata)["target_directory"])


def discover_bpf_include(target_dir):
    candidates = {}
    for output in target_dir.glob("**/build/xdpcore-sys-*/output"):
        try:
            contents = output.read_text()
        except OSError:
            continue
        match = BPF_INCLUDE_PATTERN.search(contents)
        if match is None:
            continue
        include = Path(match.group(1)).expanduser().resolve()
        if (include / "bpf/libbpf.h").is_file():
            candidates[include] = max(candidates.get(include, 0), output.stat().st_mtime_ns)

    if not candidates:
        return None

    latest = max(candidates.values())
    newest = [include for include, modified in candidates.items() if modified == latest]
    if len(newest) != 1:
        raise SystemExit(
            "Multiple equally recent libbpf include directories were found; "
            "pass --bpf-include to select one."
        )
    return newest[0]


def resolve_bpf_include(override=None):
    if override is not None:
        include = override.expanduser().resolve()
        if not (include / "bpf/libbpf.h").is_file():
            raise SystemExit(f"BPF include directory lacks bpf/libbpf.h: {include}")
        return include

    target_dir = cargo_target_dir()
    include = discover_bpf_include(target_dir)
    if include is None:
        subprocess.check_call(["cargo", "check", "--package", "xdpcore-sys"], cwd=ROOT.parent)
        include = discover_bpf_include(target_dir)
    if include is None:
        raise SystemExit(f"Could not find a completed xdpcore-sys build under {target_dir}")
    return include


def generate_bindings(bpf_include):
    subprocess.check_call([
        "cargo", "run", "--manifest-path", str(ROOT / "tools/bindgen/Cargo.toml"),
        "--", str(bpf_include),
    ], cwd=ROOT)


def update(revision, regenerate_bindings=False, bpf_include_override=None):
    bpf_include = None
    if regenerate_bindings:
        bpf_include = resolve_bpf_include(bpf_include_override)

    with tempfile.TemporaryDirectory(prefix="xdp-tools-") as source:
        subprocess.check_call([
            "git", "clone", "--quiet", "--depth=1", "--no-checkout",
            REPOSITORY, source,
        ])
        if revision != "HEAD":
            subprocess.check_call([
                "git", "-C", source, "fetch", "--quiet", "--depth=1",
                "origin", revision,
            ])
            revision = "FETCH_HEAD"
        revision = git(source, "rev-parse", "--verify", f"{revision}^{{commit}}").decode().strip()
        subprocess.check_call([
            "git", "-C", source, "checkout", "--quiet", "--detach", revision,
        ])
        update_from_checkout(source, revision)

    if regenerate_bindings:
        generate_bindings(bpf_include)


def update_from_checkout(source, revision):
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
    set_build_rs_version(version)
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
    action.add_argument("--revision", metavar="REVISION", help="Git ref or commit to fetch")
    parser.add_argument(
        "--generate-bindings", action="store_true",
        help="regenerate src/bindings.rs after importing the revision",
    )
    parser.add_argument(
        "--bpf-include", type=Path,
        help="override the automatically discovered libbpf include directory",
    )
    args = parser.parse_args()
    if args.check:
        if args.generate_bindings or args.bpf_include is not None:
            parser.error("--check cannot be combined with binding generation options")
        verify()
    else:
        if args.bpf_include is not None and not args.generate_bindings:
            parser.error("--bpf-include requires --generate-bindings")
        update(args.revision, args.generate_bindings, args.bpf_include)
