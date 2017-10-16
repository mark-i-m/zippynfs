pub trait Filesystem {

     fn setattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<Timespec>,
        _mtime: Option<Timespec>,
        _fh: Option<u64>,
        _crtime: Option<Timespec>,
        _chgtime: Option<Timespec>,
        _bkuptime: Option<Timespec>,
        _flags: Option<u32>,
        reply: ReplyAttr
        ) { ... }

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
