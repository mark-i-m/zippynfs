
#[macro_use]
extern crate clap;
extern crate try_from;

extern crate zippyrpc;
extern crate client;

use try_from::{TryFrom, TryInto};
use std::process::exit;

use client::new_client;

use zippyrpc::*;

/// All valid NFS commands.
///
/// There is an impl of `TryFrom<&str>` so that we can easily attempt
/// to parse command line args.
enum NfsCommand {
    Null,
    MkDir(u64, String), // (did, filename)
    RmDir(u64, String), // (did, filename)
    Lookup(u64, String), // (did, filename)
}

impl<'a> TryFrom<&'a str> for NfsCommand {
    type Err = String;

    fn try_from(value: &'a str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = value.trim().split_whitespace().collect();

        if parts.len() < 1 {
            return Err(format!("No command given"));
        }

        match parts[0] {
            "NULL" => Ok(NfsCommand::Null),
            "MKDIR" => {
                if parts.len() < 3 {
                    Err("Mkdir without new dir name or parent dir name".into())
                } else {
                    Ok(NfsCommand::MkDir(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].to_owned(),
                    ))
                }
            }
            "RMDIR" => {
                if parts.len() < 3 {
                    Err("Rmdir without new dir name or parent dir name".into())
                } else {
                    Ok(NfsCommand::RmDir(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].to_owned(),
                    ))
                }
            }
            "LOOKUP" => {
                if parts.len() < 3 {
                    Err("Lookup without parent dir name or lookup name".into())
                } else {
                    Ok(NfsCommand::Lookup(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].to_owned(),
                    ))
                }
            }
            _ => Err(format!("Unknown command: {}", value)),
        }
    }
}

/// Checks if the given string is a valid IP:port pair.
///
/// This is used for parsing command line args.
fn is_addr(arg: String) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    arg.to_socket_addrs()
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
            println!("Executing NULL");
            client.null().map_err(|e| format!("{}", e))?;
            Ok(())
        }

        NfsCommand::MkDir(did, new_dir) => {
            println!("Executing Mkdir {} {}", did, new_dir);

            // Create the RPC arguments
            let args = ZipCreateArgs::new(
                ZipDirOpArgs::new(ZipFileHandle::new(did as i64), new_dir),
                ZipSattr::new(
                    0777, // mode
                    0, // uid
                    0, // gid
                    ZipTimeVal::new(
                        0, // s
                        0, // us
                    ),
                    ZipTimeVal::new(
                        0, // s
                        0, // us
                    ),
                ),
            );

            // Send the RPC
            let res = client.mkdir(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ()).map_err(|e| format!("{}", e))
        }

        NfsCommand::Lookup(did, fname) => {
            println!("Executing Lookup {} {}", did, fname);

            // Create the RPC args
            let args = ZipDirOpArgs::new(ZipFileHandle::new(did as i64), fname);

            // Send the RPC
            let res = client.lookup(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ()).map_err(|e| format!("{}", e))
        }

        NfsCommand::RmDir(did, fname) => {
            println!("Executing RmDir {} {}", did, fname);

            // Create the RPC args
            let args = ZipDirOpArgs::new(ZipFileHandle::new(did as i64), fname);

            // Send the RPC
            let res = client.rmdir(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ()).map_err(|e| format!("{}", e))
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
    let command = matches.value_of("command").unwrap().try_into().unwrap();

    if let Err(e) = run(server_addr, command) {
        println!("Error! {}", e);
        exit(-1);
    } else {
        println!("Success!");
    }
}
