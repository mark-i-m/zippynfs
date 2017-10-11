#[macro_use]
extern crate clap;

extern crate client;

extern crate fuse;

use std::process::exit;

use client::new_client;

fn is_addr(arg: String) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    arg.to_socket_addrs()
        .map_err(|_| "Not a valid IP:Port".to_owned())
        .map(|_| ())
}

fn main() {
    let matches = clap_app!{
        zippynfs_client =>
            (version: "1.0")
            (author: "Team Chimney")
            (about: "Client for ZippyNFS for Testing")
            (@arg server: -s --server {is_addr} +required +takes_value "The \"IP:Port\" address of the server")
    }.get_matches();

    // Get the server address
    let server_addr = matches.value_of("server").unwrap();

    if let Err(e) = run(server_addr) {
        println!("Error! {}", e);
        exit(-1);
    }
}

fn run(server_addr: &str) -> Result<(), String> {
    // build a rpc client
    let mut client = new_client(server_addr).map_err(|e| format!("{}", e))?;

    // TODO

    Ok(())
}
