//! Minimal raw AF_XDP bindings to the pinned upstream libxdp implementation.
//!
//! This crate owns native build integration and the selected C ABI. It does
//! not provide safe socket, UMEM, or ring ownership; use `xsk-core` for those.
//! All functions are unsafe C calls with the original upstream contracts.
//!
//! Ring helpers call upstream's exported implementations, including their
//! acquire/release operations and cached producer/consumer index handling.

// Bindgen retains the upstream C names so declarations can be audited against
// the corresponding headers without a second naming translation.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

// Retain the single Cargo-managed native libbpf provider and its link metadata.
extern crate libbpf_sys as _;

include!("bindings.rs");
