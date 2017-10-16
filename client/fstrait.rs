pub trait Filesystem {

   fn mkdir(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        reply: ReplyEntry
        ) { ... }

    fn unlink(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        reply: ReplyEmpty
        ) { ... }

    fn rmdir(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        reply: ReplyEmpty
        ) { ... }

    fn rename(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _newparent: u64,
        _newname: &OsStr,
        reply: ReplyEmpty
        ) { ... }

    fn write(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: u64,
        _data: &[u8],
        _flags: u32,
        reply: ReplyWrite
        ) { ... }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) { ... }

    fn create(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _flags: u32,
        reply: ReplyCreate
        ) { ... }

    // needed for commit

    fn flush(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty
        ) { ... }

    fn fsync(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _datasync: bool,
        reply: ReplyEmpty
        ) { ... }


}
