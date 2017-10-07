
extern crate thrift;

use std::fs::create_dir;
use std::path::Path;

use zippyrpc::{ZippynfsSyncHandler, ZipFileHandle, ZipAttrStat, ZipSattrArgs, ZipDirOpArgs,
               ZipDirOpRes, ZipReadArgs, ZipReadRes, ZipWriteArgs, ZipCreateArgs, ZipRenameArgs,
               ZipStat, ZipReadDirRes, ZipStatFsRes, ZipCommitArgs, ZipCommitRes};

/// A server to handle RPC calls
pub struct ZippynfsServer<P: AsRef<Path>> {
    data_dir: P,
}

impl<P: AsRef<Path>> ZippynfsServer<P> {
    /// Returns a new ZippynfsServer
    pub fn new(data_dir: P) -> ZippynfsServer<P> {
        ZippynfsServer { data_dir }
    }

    /// Returns the host file path associated with the given file handle.
    fn get_path(&self, f: ZipFileHandle) -> Result<String, String> {
        Err("Unimplemented".into())
    }
}

impl<P: AsRef<Path>> ZippynfsSyncHandler for ZippynfsServer<P> {
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
        info!("Handling Mkdir");
        info!("{:?}", fsargs);

        // Get the path associated with the given file handle
        let parent = self.get_path(fsargs.where_.dir)?;

        // TODO: we ought to do something with inode/generation numbers here...

        // Create a new directory
        let new_dir = format!("{}/{}", parent, fsargs.where_.filename);
        create_dir(new_dir)?;

        // TODO: set attrs?

        // Create the return value
        // TODO
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
