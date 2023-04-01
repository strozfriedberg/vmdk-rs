use std::{
    env,
    fs,
    str,
    process::Command
};
use std::io::Write;

fn remove_inner_attrs(file: &str) {
    let body = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => panic!("Could not read '{file}': {e}"),
    };
    let mut out_file = match fs::File::create(file) {
        Ok(f) => f,
        Err(e) => panic!("Could not create '{file}': {e}"),
    };

    for line in body.lines() {
        if !line.starts_with("#![") {
            if let Err(e) = writeln!(out_file, "{}", line) {
                panic!("Could not write '{line}' into {file}: {e}");
            }
        }
    }
}

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
            .args(["--target", "rust", "-d", out_dir.to_str().unwrap(), "ksy/*.ksy"])
            .output()
            .expect("failed to execute process");
    let errors = output.stderr;
    if !errors.is_empty() {
        panic!("{}", str::from_utf8(&errors).unwrap());
    }

    if let Ok(entries) = fs::read_dir(out_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                remove_inner_attrs(entry.path().to_str().unwrap());
            }
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ksy/*.ksy");
}
