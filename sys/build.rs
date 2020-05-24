use bindgen;
use std::env;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

fn main() {
    let var = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root_dir = Path::new(&var).join("lib-common");
    let src_dir = root_dir.join("src");

    println!("cargo:rustc-link-search={}", src_dir.to_str().unwrap());
    println!("cargo:rustc-link-lib=static=libcommon");
    println!("cargo:rustc-link-lib=static=libcommon-iop");
    println!("cargo:rustc-link-lib=static=libcommon-minimal");
    println!("cargo:rustc-link-lib=ssl");
    println!("cargo:rustc-link-lib=crypto");
    println!("cargo:rustc-link-lib=xml2");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .clang_arg(format!("-I{}", root_dir.to_str().unwrap()))
        .clang_arg(format!("-I{}", src_dir.join("compat").to_str().unwrap()))
        .whitelist_function("ps_init")
        .whitelist_function("module_require")
        .whitelist_function("module_release")
        // For crate 'el'
        .whitelist_function("el_timer_register_d")
        .whitelist_function("el_unref")
        .whitelist_function("el_unregister")
        .whitelist_function("el_blocker_register")
        .whitelist_function("el_loop")
        .whitelist_function("el_loop_timeout")
        .whitelist_function("el_has_pending_events")
        // For crate 'ic'
        .whitelist_function("ic_get_module")
        // msg
        .whitelist_function("ic_msg_new")
        .whitelist_function("ic_msg_new_for_reply")
        .whitelist_function("ic_queue_for_reply")
        .whitelist_function("ic_msg_delete")
        .whitelist_function("__ic_query")
        // rpc-register
        .whitelist_function("qhash_init")
        .whitelist_function("_ic_register")
        // server
        .whitelist_function("addr_parse_minport")
        .whitelist_function("addr_info")
        .whitelist_function("ic_listento")
        .whitelist_function("ic_spawn")
        // client
        .whitelist_function("ic_init")
        .whitelist_function("ic_connect")
        .whitelist_function("ic_connect_blocking")
        .whitelist_function("ic_disconnect")
        .whitelist_function("ic_wipe")
        // Doctests are otherwise generated, which fails due to
        // possibly invalid doxygen comments.
        .generate_comments(false)
        .generate()
        .expect("Unable to generate bindings");

    let path = Path::new("./lib.rs");
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap();
    let mut writer = BufWriter::new(file);

    writer
        .write(
            b"
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#[link(name=\"libcommon\", kind=\"static\")]
#[link(name=\"libcommon-iop\", kind=\"static\")]
#[link(name=\"libcommon-minimal\", kind=\"static\")]
extern \"C\" {}
    ",
        )
        .unwrap();
    writer.flush().unwrap();

    bindings
        .write(Box::new(writer))
        .expect("Couldn't write bindings!");
}
