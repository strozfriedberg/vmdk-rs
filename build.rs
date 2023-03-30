use std::{
    env,
    str,
    process::Command
};

fn main() {
    let env_var_compiler_name = "KAITAI_STRUCT_COMPILER";
    let kaitai_struct_compiler = env::var_os(env_var_compiler_name)
        .expect(format!("Not defined env var '{env_var_compiler_name}'").as_str())
        .to_str()
        .unwrap()
        .to_string();
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let cmd_is_batch = kaitai_struct_compiler.ends_with(".bat");
    let mut cmd = if cmd_is_batch {
        Command::new("cmd")
    } else {
        Command::new(&kaitai_struct_compiler)
    };
    println!("kaitai_struct_compiler: {kaitai_struct_compiler}");

    if cmd_is_batch {
        cmd.args(["/C", &kaitai_struct_compiler]);
    };
    let output = cmd
            .args(["--target", "rust", "-d", out_dir.to_str().unwrap(), "ksy/sparse.ksy", "ksy/vmware_vmdk.ksy"])
            .output()
            .expect("failed to execute process");
    let errors = output.stderr;
    if !errors.is_empty() {
        panic!("{}", str::from_utf8(&errors).unwrap());
    }

    println!("cargo:rerun-if-changed=build.rs");
}
