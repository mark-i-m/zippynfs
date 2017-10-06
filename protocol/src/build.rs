use std::process::{Command, exit};

fn main() {
    let out = Command::new("../thrift/compiler/cpp/thrift")
        .args(&["--gen", "rs"])
        .args(&["-out", "src/"])
        .args(&["src/zippynfs.thrift"])
        .output()
        .unwrap();

    if !out.status.success() {
        println!("{}", String::from_utf8_lossy(&out.stderr));
        exit(-1);
    }

    println!("cargo:rerun-if-changed=\"{}\"", "src/zippynfs.thrift");
    //println!("cargo:rerun-if-changed=\"{}\"", "src/zippynfs.rs");
}
