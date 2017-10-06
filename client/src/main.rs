#[macro_use] extern crate clap;

use std::net::ToSocketAddrs;

const COMMANDS: &[&'static str] = &["NULL"];

fn is_addr(arg: String) -> Result<(), String> {
    arg
        .to_socket_addrs()
        .map_err(|_| "Not a valid IP:Port".to_owned())
        .map(|_| ())
}

fn is_nfs_command(arg: String) -> Result<(), String> {
    let trimmed = arg.trim();

    if COMMANDS.iter().any(|c| trimmed.starts_with(c)) {
        Ok(())
    } else {
        Err(format!("Not a valid command. Valid commands are {:?}", COMMANDS))
    }
}

fn main() {
    let matches = clap_app!{
        zippynfs_client =>
            (version: "1.0")
            (author: "Team Chimney")
            (about: "Client for ZippyNFS for Testing")
            (@arg server: -s --server {is_addr} +required +takes_value "The \"IP:Port\" address of the server")
            (@arg command: -c --command {is_nfs_command} +required +takes_value "A String representing an NFS command")
    }.get_matches();


}
