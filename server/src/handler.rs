
extern crate thrift;

use zippyrpc::{ZippynfsSyncHandler, ZipFileHandle, ZipAttrStat, ZipSattrArgs, ZipDirOpArgs,
               ZipDirOpRes, ZipReadArgs, ZipReadRes, ZipWriteArgs, ZipCreateArgs, ZipRenameArgs,
               ZipStat, ZipReadDirRes, ZipStatFsRes, ZipCommitArgs, ZipCommitRes};

/// A server to handle RPC calls
pub struct ZippynfsServer;

impl ZippynfsServer {
    pub fn new() -> ZippynfsServer {
        ZippynfsServer
    }
}

impl ZippynfsSyncHandler for ZippynfsServer {
    fn handle_null(&self) -> thrift::Result<()> {
        info!("Handling NULL");
        Ok(())
    }

    fn handle_getattr(&self, fhandle: ZipFileHandle) -> thrift::Result<ZipAttrStat> {
        Err("Unimplemented".into())
    }

    fn handle_setattr(&self, fsargs: ZipSattrArgs) -> thrift::Result<ZipAttrStat> {
        Err("Unimplemented".into())
    }

    fn handle_lookup(&self, fsargs: ZipDirOpArgs) -> thrift::Result<ZipDirOpRes> {
        Err("Unimplemented".into())
    }

    fn handle_read(&self, fsargs: ZipReadArgs) -> thrift::Result<ZipReadRes> {
        Err("Unimplemented".into())
    }

    fn handle_write(&self, fsargs: ZipWriteArgs) -> thrift::Result<ZipAttrStat> {
        Err("Unimplemented".into())
    }

    fn handle_create(&self, fsargs: ZipCreateArgs) -> thrift::Result<ZipDirOpRes> {
        Err("Unimplemented".into())
    }

    fn handle_remove(&self, fsargs: ZipDirOpArgs) -> thrift::Result<ZipStat> {
        Err("Unimplemented".into())
    }

    fn handle_rename(&self, fsargs: ZipRenameArgs) -> thrift::Result<ZipStat> {
        Err("Unimplemented".into())
    }

    fn handle_mkdir(&self, fsargs: ZipCreateArgs) -> thrift::Result<ZipDirOpRes> {
        Err("Unimplemented".into())
    }

    fn handle_rmdir(&self, fsargs: ZipDirOpArgs) -> thrift::Result<ZipStat> {
        Err("Unimplemented".into())
    }

    fn handle_readdir(&self, fsargs: ZipReadArgs) -> thrift::Result<ZipReadDirRes> {
        Err("Unimplemented".into())
    }

    fn handle_statfs(&self, fhandle: ZipFileHandle) -> thrift::Result<ZipStatFsRes> {
        Err("Unimplemented".into())
    }

    fn handle_commit(&self, fsargs: ZipCommitArgs) -> thrift::Result<ZipCommitRes> {
        Err("Unimplemented".into())
    }
}
