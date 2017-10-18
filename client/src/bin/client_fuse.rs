
#[macro_use]
extern crate clap;
extern crate libc;
extern crate fuse;
extern crate time;

extern crate client;
extern crate zippyrpc;

use std::time::Duration;
use std::process::exit;
use std::thread::sleep;
use std::ffi::OsStr;
use std::string::String;
use std::option::Option;
use std::vec::Vec;
use std::path::Path;
use std::cmp::min;

use time::{Timespec, get_time};
use fuse::{FileAttr, FileType, Filesystem, Request, ReplyAttr, ReplyCreate, ReplyData,
           ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyStatfs, ReplyWrite, ReplyOpen};

use libc::{ENOENT, ENOSYS, ENOTEMPTY, ENOTDIR, EISDIR, EEXIST, ENAMETOOLONG, EIO, EAGAIN, c_int};

use zippyrpc::*;
use client::{new_client, ZnfsClient};

const TTL: Timespec = Timespec { sec: 1, nsec: 0 }; // 1 second
const MAX_TRIES: usize = 5;

macro_rules! fn_not_impl {
    ($r:ident, $name:expr) => {
        println!("{}:Function not implimented",$name);
        $r.error(ENOSYS);
    }
}
macro_rules! errors {
    ($e:ident, $s:ident) =>{ {
        match $e {
            ZipError::Nfs(ZipErrorType::NFSERR_STALE, msg) =>{
                println!("NFS stale file handle: {}", msg);
                (false, Some(ENOENT))
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, msg) =>{
                println!("NFS no such dir or file: {}",msg);
                (false, Some(ENOENT))
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NOTEMPTY, msg) =>{
                println!("NFS Directory not empty: {}", msg);
                (false, Some(ENOTEMPTY))
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NAMETOOLONG, msg) =>{
                println!("NFS File name too long: {}", msg);
                (false, Some(ENAMETOOLONG))
            }

            ZipError::Nfs(ZipErrorType::NFSERR_ISDIR, msg) =>{
                println!("NFS Is a directory: {}", msg);
                (false, Some(EISDIR))
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NOTDIR, msg) =>{
                println!("NFS Not a directory: {}", msg);
                (false, Some(ENOTDIR))
            }

            ZipError::Nfs(ZipErrorType::NFSERR_EXIST, msg) =>{
                println!("NFS File exists: {}", msg);
                (false, Some(EEXIST))
            }

            ZipError::Transport(te) => {
                println!("Transport error... {:?}", te);
                match new_client(&$s.server_addr) {
                    Ok(client) => { $s.znfs = client; },
                    Err(_) => {}
                }
                (true, None)
            }

            err => {
                println!("Some other error: {:?}", err);
                (false, Some(EAGAIN)) // TODO: experimental
            }
        }
    } }
}

macro_rules! do_with_retry {
    ($self:ident, $rpc:expr) => { {
        let mut should_try = true;
        let mut tries = 0;
        let mut result: Option<Result<_, c_int>> = None;

        while should_try && tries < MAX_TRIES {
            // Attempt to make the RPC call
            match $rpc {
                Ok(val) => {
                    result = Some(Ok(val));
                    should_try = false;
                }
                Err(e) => {
                    let (st, le) = errors!(e, $self);

                    // XOR: should retry if there was an err returned,
                    // but if no error is returned, then we should retry.
                    assert!(st || le.is_some());
                    assert!(!st || !le.is_some());

                    should_try = st;

                    if let Some(err) = le {
                        result = Some(Err(err));
                    }
                }
            }

            // Backoff before retrying
            if should_try {
                sleep(Duration::from_secs(1 << tries));
            }

            tries += 1;
        }

        if should_try { // Too many reties
            result = Some(Err(EIO));
        }

        // At this point `result` should contain _something_
        assert!(result.is_some());

        result.unwrap()
    } }
}

fn to_sys_time(z_time: ZipTimeVal) -> Timespec {
    Timespec {
        sec: z_time.seconds,
        nsec: (z_time.useconds * 1000) as i32,
    }
}

fn to_zip_time(s_time: Timespec) -> ZipTimeVal {
    ZipTimeVal {
        seconds: s_time.sec,
        useconds: (s_time.nsec / 1000) as i64,
    }
}

struct ZippyFileSystem {
    znfs: ZnfsClient,
    server_addr: String,
    server_gen: i64,
}

impl ZippyFileSystem {
    /// Read 0 or more bytes into the given buffer and returns the size.
    ///
    /// We will read 0 bytes if EOF.
    fn read_part(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: u64,
        size: u32,
        buf: &mut Vec<u8>,
    ) -> Result<usize, c_int> {
        println!(
            "read(ino={}, _fh={}, off={}, _size={})",
            ino,
            _fh,
            offset,
            size
        );

        let args = ZipReadArgs::new(ZipFileHandle::new(ino as i64), offset as i64, size as i64);

        let result =
            do_with_retry! {
                self,
                self.znfs.read(args.clone()).map_err(|e| e.into())
            };

        match result {
            Ok(mut resattr) => {
                let data_len = resattr.data.len();

                println!("Recv {} B", data_len);

                // Return the data
                buf.append(&mut resattr.data);

                // Return the number of bytes
                Ok(data_len)
            }
            Err(err) => Err(err),
        }
    }

    /// Write 0 or more bytes from the buffer
    fn write_part(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: u64,
        data_vec: Vec<u8>,
        _flags: u32,
    ) -> Result<(), c_int> {
        let data_len = min(data_vec.len(), MAX_BUF_LEN);

        let args = ZipWriteArgs::new(
            ZipFileHandle::new(ino as i64),
            offset as i64,
            data_len as i64,
            data_vec,
            ZipWriteStable::FILE_SYNC,
        );

        let result =
            do_with_retry! {
                self,
                self.znfs.write(args.clone()).map_err(|e| e.into())
            };

        match result {
            Ok(result) => {
                self.server_gen = result.verf;

                // TODO: Check if it was a commit type or not
                println!("Write mode = {:?}", result.committed);
                assert_eq!(result.count as usize, data_len);

                Ok(())
            }
            Err(err) => Err(err),
        }

    }
}

impl Filesystem for ZippyFileSystem {
    fn init(&mut self, _req: &Request) -> Result<(), c_int> {
        Ok(()) // For the future?
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup(parent={}, name={:?})", parent, name);

        let args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );

        let result =
            do_with_retry! {
                self,
                self.znfs.lookup(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(dopres) => {
                println!("lookup response: {:?}", dopres);
                let lres = dopres.attributes;
                let my_time = to_sys_time(lres.ctime);
                let attr: FileAttr = FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: my_time,
                    crtime: my_time,
                    kind: match lres.type_ {
                        ZipFtype::NFREG => FileType::RegularFile,
                        ZipFtype::NFDIR => FileType::Directory,
                        ZipFtype::NFNON => FileType::NamedPipe,
                        ZipFtype::NFBLK => FileType::BlockDevice,
                        ZipFtype::NFCHR => FileType::CharDevice,
                        ZipFtype::NFLNK => FileType::Symlink,
                    },
                    perm: lres.mode as u16,
                    nlink: lres.nlink as u32,
                    uid: lres.uid as u32,
                    gid: lres.gid as u32,
                    rdev: lres.rdev as u32,
                    flags: 0,
                };
                reply.entry(&TTL, &attr, 0);
            }
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr(ino={})", ino);
        let args = ZipFileHandle::new(ino as i64);
        //println!("getattr response: {:?}", res);

        let result =
            do_with_retry! {
                self,
                self.znfs.getattr(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(resattr) => {
                println!("response: {:?}", resattr);
                let lres = resattr.attributes;
                let my_time = to_sys_time(lres.ctime);
                let attr: FileAttr = FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: my_time,
                    crtime: my_time,
                    kind: match lres.type_ {
                        ZipFtype::NFREG => FileType::RegularFile,
                        ZipFtype::NFDIR => FileType::Directory,
                        ZipFtype::NFNON => FileType::NamedPipe,
                        ZipFtype::NFBLK => FileType::BlockDevice,
                        ZipFtype::NFCHR => FileType::CharDevice,
                        ZipFtype::NFLNK => FileType::Symlink,
                    },
                    perm: lres.mode as u16,
                    nlink: lres.nlink as u32,
                    uid: lres.uid as u32,
                    gid: lres.gid as u32,
                    rdev: lres.rdev as u32,
                    flags: 0,
                };
                reply.attr(&TTL, &attr);
            }
        };
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: u64,
        size: u32,
        reply: ReplyData,
    ) {
        println!(
            "read(ino={}, _fh={}, off={}, _size={})",
            ino,
            _fh,
            offset,
            size
        );

        // Need to return the exact amount of data
        let mut buf = Vec::new();

        let size = size as usize;
        let offset = offset as usize;

        while buf.len() < size {
            let so_far = buf.len();
            let to_go = size - buf.len();

            let result = self.read_part(
                _req,
                ino,
                _fh,
                (offset + so_far) as u64,
                to_go as u32,
                &mut buf,
            );

            match result {
                Err(err) => {
                    reply.error(err);
                    return;
                }
                Ok(0) => {
                    // EOF
                    break;
                }
                Ok(_) => {}
            }
        }

        reply.data(&buf);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        println!("readdir(ino={}, _fh={}, off={}", ino, _fh, offset);
        let args = ZipReadDirArgs::new(ZipFileHandle::new(ino as i64));

        let result =
            do_with_retry! {
                self,
                self.znfs.readdir(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(dir_list) => {
                if offset == 0 {
                    let mut count = 2u64;
                    reply.add(1, 0, FileType::Directory, &Path::new("."));
                    reply.add(1, 1, FileType::Directory, &Path::new(".."));
                    for entry in dir_list.entries.into_iter() {
                        reply.add(
                            entry.fid as u64,
                            count,
                            match entry.type_ {
                                ZipFtype::NFREG => FileType::RegularFile,
                                ZipFtype::NFDIR => FileType::Directory,
                                ZipFtype::NFNON => FileType::NamedPipe,
                                ZipFtype::NFBLK => FileType::BlockDevice,
                                ZipFtype::NFCHR => FileType::CharDevice,
                                ZipFtype::NFLNK => FileType::Symlink,
                            },
                            entry.fname,
                        );
                        count += 1;
                    }
                }
                reply.ok();
            }
        };
    }

    fn setattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<Timespec>,
        mtime: Option<Timespec>,
        _fh: Option<u64>,
        _crtime: Option<Timespec>,
        _chgtime: Option<Timespec>,
        _bkuptime: Option<Timespec>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {

        println!("setattr(ino={})", _ino);
        let newattrs = ZipSattr::new(
            mode.map(|m| m as i16),
            uid.map(|m| m as i64),
            gid.map(|m| m as i64),
            size.map(|m| m as i64),
            atime.map(|m| to_zip_time(m)),
            mtime.map(|m| to_zip_time(m)),
        );
        let args = ZipSattrArgs::new(ZipFileHandle::new(_ino as i64), newattrs);

        let result =
            do_with_retry! {
                self,
                self.znfs.setattr(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(resattr) => {
                let lres = resattr.attributes;
                let my_time = to_sys_time(lres.ctime);
                let attr: FileAttr = FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: my_time,
                    crtime: my_time,
                    kind: match lres.type_ {
                        ZipFtype::NFREG => FileType::RegularFile,
                        ZipFtype::NFDIR => FileType::Directory,
                        ZipFtype::NFNON => FileType::NamedPipe,
                        ZipFtype::NFBLK => FileType::BlockDevice,
                        ZipFtype::NFCHR => FileType::CharDevice,
                        ZipFtype::NFLNK => FileType::Symlink,
                    },
                    perm: lres.mode as u16,
                    nlink: lres.nlink as u32,
                    uid: lres.uid as u32,
                    gid: lres.gid as u32,
                    rdev: lres.rdev as u32,
                    flags: 0,
                };
                reply.attr(&TTL, &attr);
            }
        }
    }

    fn mkdir(&mut self, _req: &Request, parent: u64, name: &OsStr, mode: u32, reply: ReplyEntry) {
        println!(
            "mkdir(parent={}, _name={:?}, _mode={})",
            parent,
            name,
            mode,
            );

        let time_now = get_time();

        let attrs = ZipSattr::new(
            Some(mode).map(|m| m as i16),
            Some(_req.uid()).map(|m| m as i64),
            Some(_req.gid()).map(|m| m as i64),
            None,
            Some(to_zip_time(time_now)),
            Some(to_zip_time(time_now)),
        );

        let dir_args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );
        let args = ZipCreateArgs::new(dir_args, attrs);

        let result =
            do_with_retry! {
                self,
                self.znfs.mkdir(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(dopres) => {
                let lres = dopres.attributes;
                let my_time = to_sys_time(lres.ctime);
                let attr: FileAttr = FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: my_time,
                    crtime: my_time,
                    kind: match lres.type_ {
                        ZipFtype::NFREG => FileType::RegularFile,
                        ZipFtype::NFDIR => FileType::Directory,
                        ZipFtype::NFNON => FileType::NamedPipe,
                        ZipFtype::NFBLK => FileType::BlockDevice,
                        ZipFtype::NFCHR => FileType::CharDevice,
                        ZipFtype::NFLNK => FileType::Symlink,
                    },
                    perm: lres.mode as u16,
                    nlink: lres.nlink as u32,
                    uid: lres.uid as u32,
                    gid: lres.gid as u32,
                    rdev: lres.rdev as u32,
                    flags: 0,
                };
                reply.entry(&TTL, &attr, 0);
            }
        }
    }

    //TODO: Experimental can be removed if it causes wierd behaviour
    fn open(&mut self, _req: &Request, ino: u64, flags: u32, reply: ReplyOpen) {
        // since our file handles and inos are same we can safely return the
        // ino as fh and flags as such
        println!("open(ino={}, flags={})", ino, flags);
        reply.opened(ino, flags);
    }

    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        flags: u32,
        reply: ReplyCreate,
    ) {
        println!(
            "create(parent={}, _name={:?}, _mode={}, flags={})",
            parent,
            name,
            mode,
            flags,
            );

        let time_now = get_time();

        let attrs = ZipSattr::new(
            Some(mode).map(|m| m as i16),
            Some(_req.uid()).map(|m| m as i64),
            Some(_req.gid()).map(|m| m as i64),
            None,
            Some(to_zip_time(time_now)),
            Some(to_zip_time(time_now)),
        );

        let dir_args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );
        let args = ZipCreateArgs::new(dir_args, attrs);

        let result =
            do_with_retry! {
                self,
                self.znfs.create(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(dopres) => {
                let lres = dopres.attributes;
                let my_time = to_sys_time(lres.ctime);
                let attr: FileAttr = FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: my_time,
                    crtime: my_time,
                    kind: match lres.type_ {
                        ZipFtype::NFREG => FileType::RegularFile,
                        ZipFtype::NFDIR => FileType::Directory,
                        ZipFtype::NFNON => FileType::NamedPipe,
                        ZipFtype::NFBLK => FileType::BlockDevice,
                        ZipFtype::NFCHR => FileType::CharDevice,
                        ZipFtype::NFLNK => FileType::Symlink,
                    },
                    perm: lres.mode as u16,
                    nlink: lres.nlink as u32,
                    uid: lres.uid as u32,
                    gid: lres.gid as u32,
                    rdev: lres.rdev as u32,
                    flags: flags,
                };

                reply.created(&TTL, &attr, 0u64, dopres.file.fid as u64, flags);
            }
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: u64,
        data: &[u8],
        _flags: u32,
        reply: ReplyWrite,
    ) {
        println!(
            "write(ino={}, fh={}, off={}, flags={} )",
            ino,
            fh,
            offset,
            _flags
        );
        let data_len = min(data.len(), MAX_BUF_LEN);
        let mut data_vec = Vec::from(&data[0..data_len]);

        let mut sent_bytes = 0;

        while data_vec.len() > 0 {
            let to_send_len = min(data_vec.len(), MAX_BUF_LEN);

            let mut to_send = data_vec;
            data_vec = to_send.split_off(to_send_len);

            // We know that this fully writes the data
            let result = self.write_part(_req, ino, fh, offset + sent_bytes, to_send, _flags);

            if let Err(err) = result {
                reply.error(err);
                return;
            } else {
                sent_bytes += to_send_len as u64;
            }
        }

        // We know that if we got here, we must have fully sent all bytes without errors
        reply.written(data_len as u32);
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        println!(
            "unlinkl( parent={}, name={:?} )",
            parent,
            name,
        );

        let args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );
        let result =
            do_with_retry! {
                self,
                self.znfs.remove(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(()) => {
                reply.ok();
            }
        }
    }

    fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        println!(
            "rmdir( parent={}, name={:?} )",
            parent,
            name,
        );

        let args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );

        let result =
            do_with_retry! {
                self,
                self.znfs.rmdir(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(()) => {
                reply.ok();
            }
        }
    }

    fn rename(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        reply: ReplyEmpty,
    ) {
        println!(
            "rename( parent={}, name={:?}, _new_parent={}, new_name={:?} )",
             parent,
            name,
           newparent,
            newname,
        );

        let old_args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );
        let new_args = ZipDirOpArgs::new(
            ZipFileHandle::new(newparent as i64),
            newname.to_os_string().into_string().unwrap(),
        );

        let args = ZipRenameArgs::new(old_args, new_args);
        let result =
            do_with_retry! {
                self,
                self.znfs.rename(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(()) => {
                reply.ok();
            }
        }
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        println!("StatFs(ino={})", _ino);

        let args = ZipFileHandle::new(_ino as i64);

        let result =
            do_with_retry! {
                self,
                self.znfs.statfs(args.clone()).map_err(|e| e.into())
            };

        match result {
            Err(err) => reply.error(err),
            Ok(result) => {
                reply.statfs(
                    result.blocks as u64,
                    result.bfree as u64,
                    result.bavail as u64,
                    1 as u64,
                    1 as u64,
                    result.bsize as u32,
                    256u32,
                    result.tsize as u32,
                );
            }
        }
    }

    // TODO: Impliment the following
    // needed for commit

    fn flush(&mut self, _req: &Request, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        fn_not_impl!(reply, "flush");
    }

    fn fsync(&mut self, _req: &Request, _ino: u64, _fh: u64, _datasync: bool, reply: ReplyEmpty) {
        fn_not_impl!(reply, "fsync");
    }
}

// Checks if the given address is a valid IP Addr
fn is_addr(arg: String) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    arg.to_socket_addrs()
        .map_err(|_| "Not a valid IP:Port".to_owned())
        .map(|_| ())
}

fn main() {
    let matches = clap_app!{
        zippynfs_client =>
            (version: "1.0")
            (author: "Team Chimney")
            (about: "Client for ZippyNFS for Testing")
            (@arg server: -s --server {is_addr} +required +takes_value "\"IPAddr:Port\" for server")
            (@arg mount: -m --mount +required +takes_value "The mount path for the nfs client")
    }.get_matches();

    // Get the server address
    let server_addr = matches.value_of("server").unwrap();

    let mnt_path = matches.value_of("mount").unwrap();

    if let Err(e) = run(server_addr, mnt_path) {
        println!("Error! {}", e);
        exit(-1);
    }

}

fn run(server_addr: &str, mnt_path: &str) -> Result<(), String> {
    // build a rpc client
    let znfs = new_client(server_addr).map_err(|e| format!("{}", e))?;

    // Mount the file system
    // using spawn_mount method, an additional thread will be started to handle the
    // mount commands and current thread of execution will work for handling the rpc
    // setup
    let mount_path = Path::new(mnt_path);

    fuse::mount(
        ZippyFileSystem {
            znfs: znfs,
            server_addr: server_addr.to_owned(),
            server_gen: 0i64,
        },
        &mount_path,
        &[], // mount options
    ).unwrap();

    // TODO Handle mount errors

    Ok(())
}
