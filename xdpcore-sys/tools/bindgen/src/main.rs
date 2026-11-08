use std::{env, path::PathBuf};

fn main() {
    let bpf_include = env::args_os().nth(1).map(PathBuf::from).expect(
        "usage: cargo run --manifest-path tools/bindgen/Cargo.toml -- <libbpf-sys include directory>",
    );
    let bpf_include = bpf_include
        .canonicalize()
        .expect("existing libbpf-sys include directory");
    assert!(
        bpf_include.join("bpf/libbpf.h").is_file(),
        "include directory must contain bpf/libbpf.h from the selected libbpf-sys build"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let headers = root.join("vendor/xdp-tools/headers");
    let bindings = bindgen::Builder::default()
        .header(root.join("bindings.h").to_string_lossy())
        .clang_arg(format!("-I{}", headers.display()))
        .clang_arg(format!("-I{}", bpf_include.display()))
        // These helpers have external definitions in the selected upstream
        // xsk.c; this emits declarations, not translated Rust ring operations.
        .generate_inline_functions(true)
        .allowlist_type("^(xsk_ring_cons|xsk_ring_prod|xsk_socket|xsk_umem|xsk_socket_config|xsk_umem_config|xsk_umem_opts|xdp_desc|xdp_statistics)$")
        .allowlist_function("^xsk_(ring_prod__(reserve|submit|fill_addr|tx_desc|needs_wakeup)|prod_nb_free|ring_cons__(peek|cancel|release|comp_addr|rx_desc)|cons_nb_avail|umem__(create|create_opts|delete)|socket__(create_shared|delete|fd))$")
        .allowlist_var("^(XDP_PACKET_HEADROOM|XDP_USE_NEED_WAKEUP|XDP_RING_NEED_WAKEUP|XDP_STATISTICS|XSK_RING_(CONS|PROD)__DEFAULT_NUM_DESCS|XSK_UMEM__DEFAULT_(FLAGS|FRAME_HEADROOM|FRAME_SIZE|TX_METADATA_LEN))$")
        .layout_tests(false)
        .generate_comments(false)
        .use_core()
        .raw_line("// Regenerate with tools/bindgen using the pinned upstream headers.")
        .generate()
        .expect("generate the selected upstream AF_XDP bindings");
    bindings
        .write_to_file(root.join("src/bindings.rs"))
        .expect("write checked-in bindings");
}
