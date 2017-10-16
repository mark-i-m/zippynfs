
#[macro_use]
extern crate clap;
extern crate libc;
extern crate fuse;
extern crate time;

extern crate client;
extern crate zippyrpc;

use std::process::exit;
use client::{new_client, ZnfsClient};
use fuse::{FileAttr, FileType, Filesystem, Request, ReplyAttr, ReplyData, ReplyEntry,
           ReplyDirectory};
use std::path::Path;
use libc::{ENOENT, ENOSYS, ENOTEMPTY, ENOTDIR, EISDIR, EEXIST, ENAMETOOLONG, EAGAIN, EIO, c_int};

use std::time::Duration;
use std::thread::sleep;
use std::ffi::{OsStr};
use std::string::String;
use std::option::Option;

use zippyrpc::*;
use time::Timespec;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 }; // 1 second

macro_rules! errors {
    ($e:ident, $r:ident, $s:ident) =>{
        match $e {
            ZipError::Nfs(ZipErrorType::NFSERR_STALE, msg) =>{
                println!("NFS stale file handle: {}", msg);
                $r.error(ENOENT);
                return;
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, msg) =>{
                println!("NFS no such dir or file: {}",msg);
                $r.error(ENOENT);
                return;
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NOTEMPTY, msg) =>{
                println!("NFS Directory not empty: {}", msg);
                $r.error(ENOTEMPTY);
                return;
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NAMETOOLONG, msg) =>{
                println!("NFS File name too long: {}", msg);
                $r.error(ENAMETOOLONG);
                return;
            }

            ZipError::Nfs(ZipErrorType::NFSERR_ISDIR, msg) =>{
                println!("NFS Is a directory: {}", msg);
                $r.error(EISDIR);
                return;
            }

            ZipError::Nfs(ZipErrorType::NFSERR_NOTDIR, msg) =>{
                println!("NFS Not a directory: {}", msg);
                $r.error(ENOTDIR);
                return;
            }

            ZipError::Nfs(ZipErrorType::NFSERR_EXIST, msg) =>{
                println!("NFS File exists: {}", msg);
                $r.error(EEXIST);
                return;
            }

            ZipError::Transport(te) => {
                println!("Transport error... {:?}", te);
                sleep(Duration::from_secs(1));
                match new_client(&$s.server_addr) {
                    Ok(client) => {$s.znfs = client;
                        $r.error(EAGAIN);
                    },
                    Err(_) => {
                        $r.error(EIO);
                    }
                }
                return;
            }

            err => println!("Some other error: {:?}", err),
        }
    }
}

macro_rules! match_with_retry {
    ($self:ident, $reply:ident, $rpc:expr, $ok:pat => $ok_block:block) => {
        let res = $rpc;

        match res {
            $ok => $ok_block
            Err(e) => errors!(e, $reply, $self),
        }
    }
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
}

impl Filesystem for ZippyFileSystem {
    // TODO: Add functions we need for our flie system bindings

    fn init(&mut self, _req: &Request) -> Result<(), c_int> {
        Ok(()) // For the future?
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup(parent={}, name={:?})", parent, name);
        let args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );
        // println!("lookup response: {:?}", res);

        match_with_retry! {
            self, reply,

            self.znfs.lookup(args).map_err(|e| e.into()),

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
                return;
            }
        }
    }
/*
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup(parent={}, name={:?})", parent, name);
        let args = ZipDirOpArgs::new(
            ZipFileHandle::new(parent as i64),
            name.to_os_string().into_string().unwrap(),
        );
        let res = self.znfs.lookup(args).map_err(|e| e.into());
        // println!("lookup response: {:?}", res);
        match res {
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
                return;
            }
            Err(e) => errors!(e, reply, self),
        }
    }
    */

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr(ino={})", ino);
        let args = ZipFileHandle::new(ino as i64);
        let res = self.znfs.getattr(args).map_err(|e| e.into());
        println!("getattr response: {:?}", res);
        match res {
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
                return;
            }

            Err(e) => errors!(e, reply, self),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: u64,
        _size: u32,
        reply: ReplyData,
    ) {

        println!(
            "read(ino={}, _fh={}, off={}, _size={})",
            ino,
            _fh,
            offset,
            _size
        );
        let args = ZipReadArgs::new(ZipFileHandle::new(ino as i64), offset as i64, _size as i64);
        let res = self.znfs.read(args).map_err(|e| e.into());
        println!("read response: {:?}", res);
        match res {
            Ok(resattr) => {
                reply.data(resattr.data.as_slice());
                return;
            }

            Err(e) => errors!(e, reply, self),
        }
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
        let res = self.znfs.readdir(args).map_err(|e| e.into());
        println!("readdir response: {:?}", res);
        match res {
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
            Err(e) => errors!(e, reply, self),
        }
    }

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
        reply: ReplyAttr,
    ) {

        println!("setattr(ino={})", _ino);
        // TODO: Add defalut vaules for the unwrapped option types
        // TODO: Add size to Zippynfs.thrift to allow truncate
        let newattrs = ZipSattr::new(
            _mode.unwrap() as i16,
            _uid.unwrap() as i64,
            _gid.unwrap() as i64,
            to_zip_time(_atime.unwrap()),
            to_zip_time(_mtime.unwrap()),
        );
        let args = ZipSattrArgs::new(ZipFileHandle::new(_ino as i64), newattrs);
        let res = self.znfs.setattr(args).map_err(|e| e.into());
        println!("setattr response: {:?}", res);
        match res {
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
                return;
            }

            Err(e) => errors!(e, reply, self),
        }

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

    fuse::mount(ZippyFileSystem { znfs, server_addr: server_addr.to_owned() }, &mount_path, &[]).unwrap();
    // TODO Handle mount erros

    Ok(())
}
