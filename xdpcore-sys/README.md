# xdpcore-sys

Minimal, owned Rust FFI bindings and build integration for upstream libxdp.
The safe AF_XDP socket, UMEM, and queue APIs live in the parent `xsk-core` crate.

## Native libraries and compatibility

- Upstream **libxdp 1.6.0**, commit
  `2ac18faa664208e82eb1584426e52a1c6da51d35`, is built from the unmodified source
  subset in `vendor/xdp-tools`.
- **libbpf-sys 1.7 or a compatible later 1.x version** provides libbpf and its
  headers. The crate enables `vendored-libbpf`, using the same Cargo-managed
  provider as consumers such as `libbpf-rs`. It never discovers a separate
  libbpf with pkg-config or builds xdp-tools' libbpf submodule.
- libxdp and libbpf are linked statically; libelf and zlib use the linkage
  selected by the unified `libbpf-sys` features (system libraries by default).
- Initial target support is **64-bit Linux with GNU libc**. The exercised
  native target is `x86_64-unknown-linux-gnu`. Other targets, kernels, and NICs
  need their own ABI, networking, and performance validation.

The Rust declarations cover only xsk-core's AF_XDP interface. Socket and UMEM
internals remain opaque. Ring functions call upstream's exported C helpers,
preserving their cached indices, wrapping arithmetic, and acquire/release
operations. There is no Rust rewrite of the ring algorithm.

The dispatcher and both default XDP programs are built and embedded using the
upstream recipe, including for applications which inhibit default program
loading. All generated files go into Cargo's `OUT_DIR`; building does not
modify the source checkout or download native sources.

## Build and verification

Required tools are a C compiler, `make`, `pkg-config`, Clang with its BPF backend,
`m4`, and binutils (linker, archiver, and `objcopy`), plus libelf/zlib development
headers. On Debian/Ubuntu these are supplied by `build-essential pkg-config
clang m4 binutils libelf-dev zlib1g-dev`.

From the xsk-core repository:

```sh
cargo test -p xdpcore-sys
cargo test -p xsk-core --lib
cargo clippy -p xdpcore-sys --all-targets --all-features -- -D warnings
python3 xdpcore-sys/tools/vendor.py --check
```

`bash xdpcore-sys/tools/check.sh` runs these unprivileged checks together and
compiles the socket lifetime suite for a subsequent privileged run. It can be
used as a CI entry point without enabling or changing repository workflows.

The ABI test compiles a C probe from the pinned headers and compares sizes,
alignments, field offsets, and constants against the Rust declarations. Ring
tests execute the actual upstream symbols without privileges. The parent's
socket lifetime tests require a privileged Linux environment and virtual
interfaces; physical NIC testing is also required for zero-copy qualification.

`cc` honors Cargo's normal `CC`, `AR`, and `CFLAGS` configuration. `CLANG`, `M4`,
and `OBJCOPY` override the respective executable paths. Extra, shell-quoted
BPF compiler arguments can be supplied through `XDPCORE_SYS_BPF_CFLAGS`, for
example target-specific include directories or a sysroot. Cross compilation
requires matching target tools and headers; it is not qualified by the native
tests alone.

## Updating upstream or bindings

`vendor/manifest.json` records upstream provenance and SHA-256 hashes for every
imported file. Verify it without any network access from this package directory:

```sh
python3 tools/vendor.py --check
```

To import a new revision from an existing upstream Git checkout:

```sh
python3 tools/vendor.py --source /path/to/xdp-tools --revision v1.6.0
```

The import reads committed Git blobs, excluding local build products and the
libbpf submodule. Review the upstream changes and the libbpf capability defines
in `build.rs`, then regenerate the narrow Rust declarations using the
repository's maintainer tool in `tools/bindgen` (which requires libclang). Normal package
builds use the checked-in declarations and do not require bindgen or libclang.

Before releasing an update, run ABI/ring tests, xsk-core's socket lifetime
tests, Wabrix builds, and throughput/drop/CPU comparisons on supported hardware.
Record the exact libxdp/libbpf versions, kernel, NIC, and build configuration.
A compatible Cargo version range is not a claim that every future release has
already been tested.

## Licenses

The binding/build integration uses BSD-2-Clause; see `LICENSE-BSD-2-Clause`.
Upstream files retain their original copyright and SPDX notices. In particular,
the userspace library is dual LGPL-2.1/BSD-2-Clause and the embedded BPF programs
are GPL-2.0. Upstream license texts are preserved under
`vendor/xdp-tools/LICENSES`. The package's license expression includes the GPL
programs distributed with it.
