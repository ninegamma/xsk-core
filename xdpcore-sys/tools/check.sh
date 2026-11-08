#!/usr/bin/env bash
# Unprivileged native validation, suitable for CI on a supported Linux runner.
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
python3 xdpcore-sys/tools/vendor.py --check
cargo test -p xdpcore-sys
cargo test -p xsk-core --lib
cargo test -p xsk-core --all-features --no-run
cargo clippy --workspace --all-targets --all-features -- -D warnings
