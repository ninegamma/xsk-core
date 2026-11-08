This maintainer-only tool generates `src/bindings.rs` from the vendored
upstream headers using rust-bindgen 0.72.1. Normal crate builds use that
checked-in file and do not require bindgen or libclang.

From the `xdpcore-sys` directory, after building the selected `libbpf-sys`:

```sh
cargo run --manifest-path tools/bindgen/Cargo.toml -- /absolute/path/to/libbpf-sys-build/out/include
```

Use the include directory exported by the selected libbpf-sys build, not
an independently installed system libbpf. Generation requires libclang.
Run the ABI and ring tests after regeneration; their C probe checks the
layouts and constants against the same pinned upstream headers.
