
use thrift::Error;

use zippynfs::{ZipException, ZipErrorType};

/// Returns a Thrift Error with corresponding NFS error inside.
pub fn nfs_error(error: ZipErrorType) -> Error {
    ZipException {
        error,
        message: match error {
            ZipErrorType::NFSERR_STALE => "NFSERR_STALE: Stale file handle".to_owned(),
            ZipErrorType::NFSERR_EXIST => "NFSERR_EXIST: File or directory exists".to_owned(),
            ZipErrorType::NFSERR_ISDIR => "NFSERR_ISDIR: Is a directory".to_owned(),
            ZipErrorType::NFSERR_NOTDIR => "NFSERR_NOTDIR: Not a directory".to_owned(),
            ZipErrorType::NFSERR_NOTEMPTY => "NFSERR_NOTEMPTY: Directory not empty".to_owned(),
            ZipErrorType::NFSERR_NOENT => "NFSERR_NOENT: No such file or directory".to_owned(),
        },
    }.into()
}
