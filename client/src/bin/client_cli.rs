#[macro_use] extern crate clap;
extern crate try_from;

extern crate zippyrpc;
extern crate client;

use try_from::{TryFrom, TryInto};
use std::process::exit;

use client::new_client;
use zippyrpc::TZippynfsSyncClient;

/// All valid NFS commands.
///
/// There is an impl of `TryFrom<&str>` so that we can easily attempt
/// to parse command line args.
enum NfsCommand {
    Null,
}

impl<'a> TryFrom<&'a str> for NfsCommand {
    type Err = String;

    fn try_from(value: &'a str) -> Result<Self, Self::Err> {
        if value.trim().starts_with("NULL") {
            Ok(NfsCommand::Null)
        } else {
            Err(format!("Unknown command: {}", value))
        }
    }
}

/// Checks if the given string is a valid IP:port pair.
///
/// This is used for parsing command line args.
fn is_addr(arg: String) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    arg
        .to_socket_addrs()
        .map_err(|_| "Not a valid IP:Port".to_owned())
        .map(|_| ())
}

/// The main routine of the CLI.
///
/// This creates an RPC client to the appropriate server and attempts to
/// execute the given command.
fn run(server_addr: &str, command: NfsCommand) -> Result<(), String> {
    // build a rpc client
    let mut client = new_client(server_addr).map_err(|e| format!("{}", e))?;

    // Attempt to execute the appropriate command
    match command {
        NfsCommand::Null => {
            client.null().map_err(|e| format!("{}", e))?;
            Ok(())
        }
    }
}

/// The main entry point of the CLI client
/// - parses args
/// - passes args to the `run` method which does the heavy lifting
fn main() {
    // Get command line args
    let matches = clap_app!{
        zippynfs_client =>
            (version: "1.0")
            (author: "Team Chimney")
            (about: "Client for ZippyNFS for Testing")
            (@arg server: -s --server {is_addr}
                +required +takes_value "The \"IP:Port\" address of the server")
            (@arg command: -c --command
                {|s| NfsCommand::try_from(&s).map(|_|())}
                +required +takes_value "A String representing an NFS command")
    }.get_matches();

    // Get the server address
    let server_addr = matches.value_of("server").unwrap();

    // Get the NFS command
    let command = matches
        .value_of("command")
        .unwrap()
        .try_into()
        .unwrap();

    if let Err(e) = run(server_addr, command) {
        println!("Error! {}", e);
        exit(-1);
    }
}
