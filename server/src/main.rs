//! This file contains all of the stuff to create and start a server
//! listening over RPC. The actually meat of the FS is in `handler.rs`.

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate thrift;
extern crate zippyrpc;

mod handler;

use std::path::Path;
use std::process::exit;

use thrift::protocol::{TCompactInputProtocolFactory, TCompactOutputProtocolFactory};
use thrift::server::TServer;
use thrift::transport::{TFramedReadTransportFactory, TFramedWriteTransportFactory};

use zippyrpc::ZippynfsSyncProcessor;

use handler::ZippynfsServer;

/// Checks if the given string is a valid IP:port pair.
///
/// This is used for parsing command line args.
fn is_addr(arg: String) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    arg.to_socket_addrs()
        .map_err(|_| "Not a valid IP:Port".to_owned())
        .map(|_| ())
}

/// The main routine of the server.
///
/// The server sits around listening for RPC calls and then
/// acts on them.
fn run<P>(server_addr: &str, data_dir: P) -> Result<(), String>
where
    P: AsRef<Path>,
{
    // Initialize the logger
    env_logger::init();

    info!("Hello! The server is starting!");

    // Create stuff for Thrift
    let i_tran_fact = TFramedReadTransportFactory::new();
    let i_prot_fact = TCompactInputProtocolFactory::new();

    let o_tran_fact = TFramedWriteTransportFactory::new();
    let o_prot_fact = TCompactOutputProtocolFactory::new();

    // demux incoming messages
    let processor = ZippynfsSyncProcessor::new(ZippynfsServer::new());

    info!("Creating a server with 10 workers");

    // create the server and start listening
    let mut server = TServer::new(
        i_tran_fact,
        i_prot_fact,
        o_tran_fact,
        o_prot_fact,
        processor,
        10, // 10 workers
    );

    info!("Listening at {}", server_addr);

    server.listen(server_addr).map_err(|e| format!("{}", e))
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
                +required +takes_value "The \"IP:Port\" address the server is listening on")
            (@arg data_dir: -d --dir
                +required +takes_value "The directory where the server should put its FS contents")
    }.get_matches();

    // Get the server address
    let server_addr = matches.value_of("server").unwrap();

    // Get the server data dir
    let data_dir = matches.value_of("data_dir").unwrap();

    if let Err(e) = run(server_addr, data_dir) {
        println!("Error! {}", e);
        exit(-1);
    }
}
