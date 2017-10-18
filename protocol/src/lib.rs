
extern crate ordered_float;
extern crate thrift;
extern crate try_from;

#[allow(non_camel_case_types)]
mod zippynfs;

mod errors;

pub use zippynfs::*;
pub use errors::*;

/// The maximum size of a message via Thrift... this appears to be a bug in the thrift rust impl.
pub const MAX_BUF_LEN: usize = 4000;
