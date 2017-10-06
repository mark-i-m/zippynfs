//! This library contains all the common code for CLI and FUSE clients.

extern crate thrift;
extern crate zippyrpc;

use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol};
use thrift::transport::{ReadHalf, TFramedReadTransport, TFramedWriteTransport, TIoChannel,
                        TTcpChannel, WriteHalf};

use zippyrpc::ZippynfsSyncClient;

type ClientInputProtocol = TCompactInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>;
type ClientOutputProtocol = TCompactOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>;

/// Create a new thrift client communicating with the given server address.
pub fn new_client(
    server_addr: &str,
) -> thrift::Result<ZippynfsSyncClient<ClientInputProtocol, ClientOutputProtocol>> {
    let mut c = TTcpChannel::new();

    // open the underlying TCP stream
    println!("connecting to ZippyNFS server on {}", server_addr);
    c.open(server_addr)?;

    // clone the TCP channel into two halves, one which
    // we'll use for reading, the other for writing
    let (i_chan, o_chan) = c.split()?;

    // wrap the raw sockets (slow) with a buffered transport of some kind
    let i_tran = TFramedReadTransport::new(i_chan);
    let o_tran = TFramedWriteTransport::new(o_chan);

    // now create the protocol implementations
    let i_prot = TCompactInputProtocol::new(i_tran);
    let o_prot = TCompactOutputProtocol::new(o_tran);

    // we're done!
    Ok(ZippynfsSyncClient::new(i_prot, o_prot))
}
