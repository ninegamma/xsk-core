# Changelog

## [0.2.0] - 2026-09-24

Version 0.2.0 builds on the 0.1.0 hard-fork baseline and focuses on the
descriptor, queue, and UMEM memory layers needed by high-throughput AF_XDP
applications.

### Added

- Memfd-backed UMEM mappings on Linux. `Umem::backing_fd()` returns a
  duplicated descriptor that can be used to create additional `MAP_SHARED`
  aliases of the same packet-buffer pages.
- Documentation describing shared UMEM mappings, descriptor ownership, and
  frame headroom responsibilities across the kernel, driver, XDP, and user
  space.

### Changed

- `FrameDesc` now follows the native AF_XDP descriptor layout: `u64` UMEM
  offsets, `u32` lengths, and `u32` options.
- Fill and completion queue batch APIs use address slices and copy wrapped ring
  regions in at most two bulk operations.
- UMEM frame layout and address calculations now use the native descriptor
  offset model directly.
- The core API was simplified around packet descriptors. High-level packet
  parsing and packet-memory access belong to the application layer.
- The crate now uses the 2024 Rust edition and updated dependency versions.

### Removed

- The former high-level frame data, cursor, and `SegmentLengths` APIs.
- Integration tests that depended on the removed high-level frame-memory API.

## [0.1.0] - 2026-09-09

Initial `xsk-core` release after forking `xsk-rs`. This version established the
crate name and simplified AF_XDP descriptor and UMEM API used as the baseline
for the 0.2.0 changes.
