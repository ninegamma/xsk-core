use std::{
    collections::BTreeMap,
    fs,
    mem::{align_of, offset_of, size_of},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use xdpcore_sys::*;

struct ProbeDirectory(PathBuf);

impl Drop for ProbeDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rust_layouts_and_constants_match_the_compiled_upstream_headers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let directory = ProbeDirectory(std::env::temp_dir().join(format!(
        "xdpcore-abi-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir(&directory.0).unwrap();
    let executable = directory.0.join("abi-probe");
    let compiler = option_env!("XDPCORE_SYS_CC").unwrap_or("cc");
    let mut command = Command::new(compiler);
    command
        .arg("-std=gnu11")
        .arg("-I")
        .arg(root.join("vendor/xdp-tools/headers"))
        .arg("-I")
        .arg(env!("XDPCORE_SYS_BPF_INCLUDE"))
        .arg(root.join("tests/abi.c"))
        .arg("-o")
        .arg(&executable);
    let result = command.output().expect("run native C ABI probe compiler");
    assert!(
        result.status.success(),
        "C ABI probe compilation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(&executable).output().unwrap();
    assert!(result.status.success());
    let native: BTreeMap<String, usize> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            let (key, value) = line.split_once('=').unwrap();
            (key.to_owned(), value.parse().unwrap())
        })
        .collect();
    let mut rust = BTreeMap::new();
    macro_rules! layout {
        ($ty:ty; $($field:ident),* $(,)?) => {
            rust.insert(concat!(stringify!($ty), ".size").to_owned(), size_of::<$ty>());
            rust.insert(concat!(stringify!($ty), ".align").to_owned(), align_of::<$ty>());
            $(rust.insert(concat!(stringify!($ty), ".", stringify!($field)).to_owned(), offset_of!($ty, $field));)*
        };
    }
    macro_rules! constant {
        ($($name:ident),* $(,)?) => {
            $(rust.insert(stringify!($name).to_owned(), $name as usize);)*
        };
    }
    layout!(xsk_ring_prod; cached_prod, cached_cons, mask, size, producer, consumer, ring, flags);
    layout!(xsk_ring_cons; cached_prod, cached_cons, mask, size, producer, consumer, ring, flags);
    layout!(xdp_desc; addr, len, options);
    layout!(xsk_umem_config; fill_size, comp_size, frame_size, frame_headroom, flags);
    layout!(xsk_umem_opts; sz, fd, size, fill_size, comp_size, frame_size, frame_headroom, flags, tx_metadata_len);
    layout!(xsk_socket_config; rx_size, tx_size, __bindgen_anon_1, xdp_flags, bind_flags);
    layout!(xdp_statistics; rx_dropped, rx_invalid_descs, tx_invalid_descs, rx_ring_full, rx_fill_ring_empty_descs, tx_ring_empty_descs);
    constant!(
        XDP_PACKET_HEADROOM,
        XDP_USE_NEED_WAKEUP,
        XDP_RING_NEED_WAKEUP,
        XDP_STATISTICS,
        XSK_RING_CONS__DEFAULT_NUM_DESCS,
        XSK_RING_PROD__DEFAULT_NUM_DESCS,
        XSK_UMEM__DEFAULT_FLAGS,
        XSK_UMEM__DEFAULT_FRAME_HEADROOM,
        XSK_UMEM__DEFAULT_FRAME_SIZE,
        XSK_UMEM__DEFAULT_TX_METADATA_LEN,
    );
    assert_eq!(native, rust, "checked-in bindings differ from the C ABI");
}
