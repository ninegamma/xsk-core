use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const LIBXDP_VERSION: &str = "1.6.0";

// Every capability below is present in our minimum supported libbpf (1.7).
// Keep this list aligned with the compatibility branches in upstream libxdp.c.
const LIBBPF_CAPABILITIES: &[&str] = &[
    "HAVE_LIBBPF_BTF__LOAD_FROM_KERNEL_BY_ID",
    "HAVE_LIBBPF_BTF__TYPE_CNT",
    "HAVE_LIBBPF_BPF_OBJECT__NEXT_MAP",
    "HAVE_LIBBPF_BPF_OBJECT__NEXT_PROGRAM",
    "HAVE_LIBBPF_BPF_PROGRAM__INSN_CNT",
    "HAVE_LIBBPF_BPF_PROGRAM__TYPE",
    "HAVE_LIBBPF_BPF_PROGRAM__FLAGS",
    "HAVE_LIBBPF_BPF_MAP_CREATE",
    "HAVE_LIBBPF_BPF_XDP_ATTACH",
];

fn run(command: &mut Command, description: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{description}: {command:?}: {error}"));
    assert!(
        output.status.success(),
        "{description}: {command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn tool(variable: &str, fallback: &str) -> OsString {
    println!("cargo:rerun-if-env-changed={variable}");
    env::var_os(variable).unwrap_or_else(|| fallback.into())
}

fn compiler_query(compiler: &cc::Tool, option: &str) -> Option<String> {
    let output = compiler.to_command().arg(option).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let answer = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!answer.is_empty()).then_some(answer)
}

fn compile_bpf(
    source: &Path,
    stem: &str,
    out: &Path,
    headers: &Path,
    bpf_headers: &Path,
    compiler: &cc::Tool,
) -> PathBuf {
    let clang = tool("CLANG", "clang");
    let objcopy = env::var_os("OBJCOPY").unwrap_or_else(|| {
        compiler_query(compiler, "-print-prog-name=objcopy")
            .unwrap_or_else(|| "objcopy".into())
            .into()
    });
    let bpf_target = match env::var("CARGO_CFG_TARGET_ENDIAN").unwrap().as_str() {
        "little" => "bpfel",
        "big" => "bpfeb",
        other => panic!("unsupported target byte order: {other}"),
    };
    let object = format!("{stem}.o");
    let embedded = format!("{stem}.embed.o");
    let mut command = Command::new(clang);
    command
        .current_dir(out)
        .args([
            "-target",
            bpf_target,
            "-O2",
            "-g",
            "-c",
            "-fno-stack-protector",
        ])
        .args(["-Wall", "-Werror", "-Wno-visibility", "-Wno-unused-value"])
        .args(["-Wno-pointer-sign", "-Wno-compare-distinct-pointer-types"])
        .arg("-I")
        .arg(bpf_headers)
        .arg("-I")
        .arg(headers);

    // Clang's BPF target does not discover the target C compiler's multiarch
    // asm headers. Query that compiler, rather than assuming an x86 host.
    if let Some(multiarch) = compiler_query(compiler, "-print-multiarch") {
        let sysroot = compiler_query(compiler, "-print-sysroot").unwrap_or_else(|| "/".into());
        let include = Path::new(&sysroot).join("usr/include").join(multiarch);
        if include.is_dir() {
            command.arg("-idirafter").arg(include);
        }
    }
    println!("cargo:rerun-if-env-changed=XDPCORE_SYS_BPF_CFLAGS");
    if let Ok(flags) = env::var("XDPCORE_SYS_BPF_CFLAGS") {
        command.args(shlex::split(&flags).expect("invalid quoting in XDPCORE_SYS_BPF_CFLAGS"));
    }
    run(
        command.arg(source).arg("-o").arg(&object),
        "compile embedded BPF program",
    );

    // Match upstream's embedding recipe. Relative input names are intentional:
    // they determine the _binary_* symbols referenced by libxdp.c.
    run(
        compiler
            .to_command()
            .current_dir(out)
            .args([
                "-r",
                "-nostdlib",
                "-Wl,-z,noexecstack,--format=binary",
                "-o",
            ])
            .arg(&embedded)
            .arg(&object),
        "embed BPF object",
    );
    run(
        Command::new(objcopy)
            .current_dir(out)
            .args([
                "--rename-section",
                ".data=.rodata,alloc,load,readonly,data,contents",
            ])
            .arg(&embedded),
        "mark embedded BPF object read-only",
    );
    out.join(embedded)
}

fn main() {
    assert_eq!(
        env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("linux"),
        "xdpcore-sys requires Linux"
    );
    assert_eq!(
        env::var("CARGO_CFG_TARGET_POINTER_WIDTH").as_deref(),
        Ok("64"),
        "xdpcore-sys currently supports 64-bit targets"
    );
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ENV").as_deref(),
        Ok("gnu"),
        "xdpcore-sys currently supports GNU libc targets"
    );

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let vendor = manifest.join("vendor/xdp-tools");
    let headers = vendor.join("headers");
    let sources = vendor.join("lib/libxdp");
    let bpf_headers = PathBuf::from(
        env::var_os("DEP_BPF_INCLUDE").expect("libbpf-sys must export DEP_BPF_INCLUDE"),
    );
    assert!(
        bpf_headers.join("bpf/libbpf.h").is_file(),
        "libbpf-sys did not provide its headers"
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-env-changed=OBJCOPY");

    let mut build = cc::Build::new();
    build
        .include(&bpf_headers)
        .include(&headers)
        .include(&sources)
        .flag("-std=gnu11")
        .define("LIBXDP_STATIC", "1")
        .define("HAVE_SECURE_GETENV", None)
        .define("_LARGEFILE64_SOURCE", None)
        .define("_FILE_OFFSET_BITS", "64")
        .define("BPF_DIR_MNT", "\"/sys/fs/bpf\"")
        .define("BPF_OBJECT_PATH", "\"/usr/local/lib/bpf\"")
        .define("MAX_DISPATCHER_ACTIONS", "10")
        .define("TOOLS_VERSION", format!("\"{LIBXDP_VERSION}\"").as_str())
        .define("RUNDIR", "\"/run\"");
    for capability in LIBBPF_CAPABILITIES {
        build.define(capability, None);
    }
    let compiler = build.get_compiler();
    let dispatcher = out.join("xdp-dispatcher.c");
    let output = run(
        Command::new(tool("M4", "m4"))
            .arg("-DMAX_DISPATCHER_ACTIONS=10")
            .arg(sources.join("xdp-dispatcher.c.in")),
        "generate XDP dispatcher",
    );
    fs::write(&dispatcher, output.stdout).expect("write generated dispatcher");
    for (source, stem) in [
        (dispatcher, "xdp-dispatcher"),
        (sources.join("xsk_def_xdp_prog.c"), "xsk_def_xdp_prog"),
        (
            sources.join("xsk_def_xdp_prog_5.3.c"),
            "xsk_def_xdp_prog_5.3",
        ),
    ] {
        build.object(compile_bpf(
            &source,
            stem,
            &out,
            &headers,
            &bpf_headers,
            &compiler,
        ));
    }
    build
        .file(sources.join("libxdp.c"))
        .file(sources.join("xsk.c"))
        .compile("xdp");

    // libbpf-sys owns all bpf/elf/z linkage. Do not rediscover or link a second
    // system libbpf here; its Rust dependency carries the native link metadata.
    println!("cargo:include={}", headers.display());
    println!(
        "cargo:rustc-env=XDPCORE_SYS_BPF_INCLUDE={}",
        bpf_headers.display()
    );
    println!(
        "cargo:rustc-env=XDPCORE_SYS_CC={}",
        compiler.path().display()
    );
}
