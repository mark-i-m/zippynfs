use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    Command::new("../thrift/compiler/cpp/thrift")
        .args(&["--gen", "rs", "zippynfs.thrift"])
        .status()
        .unwrap();

    println!("cargo:rerun-if-changed={}", "zippynfs.thrift");
}
