use std::fs::metadata;
use std::process::{Command, exit};

fn main() {
    // Only regenerate the rust if it is older than the thrift spec.
    // This way, cargo will not keep rebuilding zippyrpc, which is
    // really annoying
    let meta_rs = metadata("src/zippynfs.rs");
    let meta_thrift = metadata("src/zippynfs.thrift");

    if meta_rs.is_err() || meta_thrift.is_err() ||
        meta_rs.unwrap().modified().unwrap() <= meta_thrift.unwrap().modified().unwrap()
    {
        //println!("cargo:warning=\"Recompiling Thrift\"");

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
    } else {
        //println!("cargo:warning=\"Thrift Up to Date :)\"");
    }

    //println!("cargo:rerun-if-changed=\"src/zippynfs.thrift\"");
}
