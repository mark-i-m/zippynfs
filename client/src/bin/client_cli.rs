
#[macro_use]
extern crate clap;
extern crate try_from;

extern crate zippyrpc;
extern crate client;
extern crate thrift;

use std::process::exit;

use try_from::{TryFrom, TryInto};

use client::new_client;

use zippyrpc::*;

/// All valid NFS commands.
///
/// There is an impl of `TryFrom<&str>` so that we can easily attempt
/// to parse command line args.
enum NfsCommand {
    Null,
    MkDir(u64, String), // (did, filename)
    Remove(u64, String), // (did, filename)
    RmDir(u64, String), // (did, filename)
    Lookup(u64, String), // (did, filename)
    ReadDir(u64, u64), // did, offset
    GetAttr(u64), // fid
    SetAttr(u64, u64, u64, u64), // fid, size, atime, mtime
    Read(u64, u64, u64), // fid, offset, count
    Write(u64, u64, u64, bool, String), // fid, offset, count, stable, data
    Create(u64, String), // did, name
    Rename(u64, String, u64, String), // from_did, from_name, to_did, to_name
    StatFs,
    Commit(u64, u64, u64), // fid, offset, count
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
            "REMOVE" => {
                if parts.len() < 3 {
                    Err("Remove without new dir name or parent dir name".into())
                } else {
                    Ok(NfsCommand::Remove(
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
            "READDIR" => {
                if parts.len() < 3 {
                    Err("Readdir without dir name".into())
                } else {
                    Ok(NfsCommand::ReadDir(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].parse().map_err(|e| format!("{}", e))?,
                    ))
                }
            }
            "GETATTR" => {
                if parts.len() < 2 {
                    Err("GetAttr without fid".into())
                } else {
                    Ok(NfsCommand::GetAttr(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                    ))
                }
            }
            "SETATTR" => {
                if parts.len() < 5 {
                    Err("SetAttr without fid, size, atime, mtime".into())
                } else {
                    Ok(NfsCommand::SetAttr(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].parse().map_err(|e| format!("{}", e))?,
                        parts[3].parse().map_err(|e| format!("{}", e))?,
                        parts[4].parse().map_err(|e| format!("{}", e))?,
                    ))
                }
            }
            "READ" => {
                if parts.len() < 4 {
                    Err("Read without fid, offset, count".into())
                } else {
                    Ok(NfsCommand::Read(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].parse().map_err(|e| format!("{}", e))?,
                        parts[3].parse().map_err(|e| format!("{}", e))?,
                    ))
                }
            }
            "WRITE" => {
                if parts.len() < 6 {
                    Err("Write without fid, offset, count, stable, data".into())
                } else {
                    Ok(NfsCommand::Write(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].parse().map_err(|e| format!("{}", e))?,
                        parts[3].parse().map_err(|e| format!("{}", e))?,
                        parts[4].parse().map_err(|e| format!("{}", e))?,
                        parts[5].to_owned(),
                    ))
                }
            }
            "CREATE" => {
                if parts.len() < 3 {
                    Err("Create without did, fname".into())
                } else {
                    Ok(NfsCommand::Create(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].to_owned(),
                    ))
                }
            }
            "RENAME" => {
                if parts.len() < 5 {
                    Err("Rename without fromdid, fromfname, todid, tofname".into())
                } else {
                    Ok(NfsCommand::Rename(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].to_owned(),
                        parts[3].parse().map_err(|e| format!("{}", e))?,
                        parts[4].to_owned(),
                    ))
                }
            }
            "STATFS" => Ok(NfsCommand::StatFs),
            "COMMIT" => {
                if parts.len() < 4 {
                    Err("Commit without fid, offset, count".into())
                } else {
                    Ok(NfsCommand::Commit(
                        parts[1].parse().map_err(|e| format!("{}", e))?,
                        parts[2].parse().map_err(|e| format!("{}", e))?,
                        parts[3].parse().map_err(|e| format!("{}", e))?,
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
fn run(server_addr: &str, command: NfsCommand) -> Result<(), ZipError> {
    // build a rpc client
    let mut client = new_client(server_addr)?;

    // Attempt to execute the appropriate command
    match command {
        NfsCommand::Null => {
            println!("Executing NULL");
            client.null()?;
            Ok(())
        }

        NfsCommand::StatFs => {
            println!("Executing STATFS");
            client.statfs(ZipFileHandle::new(1))?;
            Ok(())
        }

        NfsCommand::MkDir(did, new_dir) => {
            println!("Executing Mkdir {} {}", did, new_dir);

            // Create the RPC arguments
            let args = ZipCreateArgs::new(
                ZipDirOpArgs::new(ZipFileHandle::new(did as i64), new_dir),
                ZipSattr::new(
                    None, // mode
                    None, // size
                    None, // uid
                    None, // gid
                    None, // atime
                    None, // mtime
                ),
            );

            // Send the RPC
            let res = client.mkdir(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::Lookup(did, fname) => {
            println!("Executing Lookup {} {}", did, fname);

            // Create the RPC args
            let args = ZipDirOpArgs::new(ZipFileHandle::new(did as i64), fname);

            // Send the RPC
            let res = client.lookup(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::Remove(did, fname) => {
            println!("Executing Remove {} {}", did, fname);

            // Create the RPC args
            let args = ZipDirOpArgs::new(ZipFileHandle::new(did as i64), fname);

            // Send the RPC
            let res = client.remove(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::RmDir(did, fname) => {
            println!("Executing RmDir {} {}", did, fname);

            // Create the RPC args
            let args = ZipDirOpArgs::new(ZipFileHandle::new(did as i64), fname);

            // Send the RPC
            let res = client.rmdir(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::ReadDir(did, offset) => {
            println!("Executing ReadDir {} {}", did, offset);

            // Create the RPC args
            let args = ZipReadDirArgs::new(ZipFileHandle::new(did as i64), offset as i64);

            // Send the RPC
            let res = client.readdir(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::GetAttr(fid) => {
            println!("Executing GetAttr {}", fid);

            // Create the RPC args
            let args = ZipFileHandle::new(fid as i64);

            // Send the RPC
            let res = client.getattr(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::SetAttr(fid, size, atime, mtime) => {
            println!("Executing SetAttr {} {} {} {}", fid, size, atime, mtime);

            // Create the RPC args
            let args = ZipSattrArgs::new(
                ZipFileHandle::new(fid as i64),
                ZipSattr::new(
                    None,
                    None,
                    None,
                    size as i64,
                    ZipTimeVal::new(atime as i64, 0),
                    ZipTimeVal::new(mtime as i64, 0),
                ),
            );

            // Send the RPC
            let res = client.setattr(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::Read(fid, offset, count) => {
            println!("Executing Read {} {} {}", fid, offset, count);

            // Create the RPC args
            let args =
                ZipReadArgs::new(ZipFileHandle::new(fid as i64), offset as i64, count as i64);

            // Send the RPC
            let res = client.read(args);

            // Check the result
            println!("Received response: {:?}", res);

            if let Ok(ref resp) = res {
                let string = String::from_utf8_lossy(&resp.data);
                println!("Read data:\n======\n{}\n======\n", string);
            }

            res.map(|_| ())
        }

        NfsCommand::Write(fid, offset, count, stable, data) => {
            println!(
                "Executing Write {} {} {} {} {}",
                fid,
                offset,
                count,
                stable,
                data
            );

            // Create the RPC args
            let args = ZipWriteArgs::new(
                ZipFileHandle::new(fid as i64),
                offset as i64,
                count as i64,
                data.into(),
                if stable {
                    ZipWriteStable::FILE_SYNC
                } else {
                    ZipWriteStable::UNSTABLE
                },
            );

            // Send the RPC
            let res = client.write(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::Create(did, fname) => {
            println!("Executing Create {} {}", did, fname);

            // Create the RPC args
            let args = ZipCreateArgs::new(
                ZipDirOpArgs::new(ZipFileHandle::new(did as i64), fname),
                ZipSattr::new(None, None, None, None, None, None),
            );

            // Send the RPC
            let res = client.create(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::Rename(fdid, ffname, tdid, tfname) => {
            println!("Executing Rename {} {} {} {}", fdid, ffname, tdid, tfname);

            // Create the RPC args
            let args = ZipRenameArgs::new(
                ZipDirOpArgs::new(ZipFileHandle::new(fdid as i64), ffname),
                ZipDirOpArgs::new(ZipFileHandle::new(tdid as i64), tfname),
            );

            // Send the RPC
            let res = client.rename(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }

        NfsCommand::Commit(fid, offset, count) => {
            println!("Executing Rename {} {} {}", fid, offset, count);

            // Create the RPC args
            let args =
                ZipCommitArgs::new(ZipFileHandle::new(fid as i64), offset as i64, count as i64);

            // Send the RPC
            let res = client.commit(args);

            // Check the result
            println!("Received response: {:?}", res);

            res.map(|_| ())
        }
    }.map_err(|e| e.into())
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
        println!("Error! {:?}", e);
        exit(-1);
    } else {
        println!("Success!");
    }
}
