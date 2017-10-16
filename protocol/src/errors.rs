
use thrift::{Error, TransportError, ProtocolError, ApplicationError};

use zippynfs::{ZipException, ZipErrorType};

/// Returns a Thrift Error with corresponding NFS error inside.
pub fn nfs_error(error: ZipErrorType) -> Error {
    ZipException {
        error: Box::new(error),
        message: match error {
            ZipErrorType::NFSERR_STALE => "NFSERR_STALE: Stale file handle".to_owned(),
            ZipErrorType::NFSERR_EXIST => "NFSERR_EXIST: File or directory exists".to_owned(),
            ZipErrorType::NFSERR_ISDIR => "NFSERR_ISDIR: Is a directory".to_owned(),
            ZipErrorType::NFSERR_NOTDIR => "NFSERR_NOTDIR: Not a directory".to_owned(),
            ZipErrorType::NFSERR_NOTEMPTY => "NFSERR_NOTEMPTY: Directory not empty".to_owned(),
            ZipErrorType::NFSERR_NOENT => "NFSERR_NOENT: No such file or directory".to_owned(),
            ZipErrorType::NFSERR_NAMETOOLONG => "NFSERR_NAMETOOLONG: File name too long".to_owned(),
        },
    }.into()
}

#[derive(Debug)]
pub enum ZipError {
    Transport(TransportError),
    Protocol(ProtocolError),
    Application(ApplicationError),
    Nfs(ZipErrorType, String),
}

impl From<Error> for ZipError {
    fn from(result: Error) -> ZipError {
        match result {
            Error::Transport(te) => ZipError::Transport(te),
            Error::Protocol(pe) => ZipError::Protocol(pe),
            Error::Application(ae) => ZipError::Application(ae),
            Error::User(boxed) => {
                let ZipException { error, message } = *boxed.downcast().unwrap();
                ZipError::Nfs(*error, message)
            }
        }
    }
}
