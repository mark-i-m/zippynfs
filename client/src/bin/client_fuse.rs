
#[macro_use]
extern crate clap;
extern crate libc;
extern crate fuse;
extern crate time;

extern crate client;
extern crate zippyrpc;

use std::process::exit;
use client::{new_client,ZnfsClient};
use std::env;
use fuse::{FileAttr, FileType, Filesystem, Request, ReplyAttr, ReplyData, ReplyEntry, ReplyDirectory};
use std::path::Path;
use libc::{ENOENT, ENOSYS,ENOTEMPTY,ENOTDIR, EISDIR,EEXIST,ENAMETOOLONG};
use std::ffi::{OsStr,OsString};
use std::string::String;
use zippyrpc::*;
use time::Timespec;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };                     // 1 second

macro_rules! errors {
    ($e:ident, $r:ident) =>{
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
            err => println!("Some other error: {:?}", err),
        }
    }
}

fn to_sys_time( zTime: ZipTimeVal) -> Timespec {
    Timespec {sec: zTime.seconds, nsec: zTime.useconds as i32 }
}

struct ZippyFileSystem {
    znfs : ZnfsClient,
}
impl Filesystem for ZippyFileSystem {
    // TODO: Add functions we need for our flie system bindings

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup(parent={}, name={:?})", parent, name);
        let args = ZipDirOpArgs::new(ZipFileHandle::new(parent as i64), name.to_os_string().into_string().unwrap());
        let res = self.znfs.lookup(args).map_err(|e| e.into());
        println!("Lookup response: {:?}", res);
        match res{
            Ok(dopres)=>{
                let lres =  dopres.attributes;
                let myTime =  to_sys_time(lres.ctime);
                let attr : FileAttr =  FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: myTime,
                    crtime: myTime,
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
                reply.entry(&TTL,&attr,0);
                return;
            }
            Err(e) => errors!(e,reply),
       }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr(ino={})", ino);
        let args = ZipFileHandle::new(ino as i64);
        let res = self.znfs.getattr(args).map_err(|e| e.into());
        println!("Lookup response: {:?}", res);
        match res{
            Ok(resattr)=>{
                let lres =  resattr.attributes;
                let myTime =  to_sys_time(lres.ctime);
                let attr : FileAttr =  FileAttr {
                    ino: lres.fid as u64,
                    size: lres.size as u64,
                    blocks: lres.blocks as u64,
                    atime: to_sys_time(lres.atime),
                    mtime: to_sys_time(lres.mtime),
                    ctime: myTime,
                    crtime: myTime,
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
                reply.attr(&TTL,&attr);
                return;
            }

            Err(e) => errors!(e,reply),
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, _size: u32, reply: ReplyData) {

        println!("read(ino={}, _fh={}, off={}, _size={})", ino, _fh, offset, _size);
        reply.error(ENOSYS)
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        println!("readdir(ino={}, _fh={}, off={}", ino, _fh, offset);
        reply.error(ENOSYS)
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
    let mut znfs = new_client(server_addr).map_err(|e| format!("{}", e))?;

    // Mount the file system
    // using spawn_mount method, an additional thread will be started to handle the
    // mount commands and current thread of execution will work for handling the rpc
    // setup
    let mount_path = Path::new(mnt_path);

    fuse::mount(ZippyFileSystem{znfs}, &mount_path,&[]).unwrap();
    // TODO Handle mount erros

    Ok(())
}
