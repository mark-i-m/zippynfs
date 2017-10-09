
extern crate thrift;

mod counter;

use std::fs::{create_dir, read_dir};
use std::path::{Path, PathBuf};
use std::collections::{HashSet, VecDeque};

use regex::Regex;

use self::counter::AtomicPersistentUsize;

use zippyrpc::{ZippynfsSyncHandler, ZipFileHandle, ZipAttrStat, ZipSattrArgs, ZipDirOpArgs,
               ZipDirOpRes, ZipReadArgs, ZipReadRes, ZipWriteArgs, ZipCreateArgs, ZipRenameArgs,
               ZipStat, ZipReadDirRes, ZipStatFsRes, ZipCommitArgs, ZipCommitRes};

type Fid = usize;

/// A server to handle RPC calls
pub struct ZippynfsServer<'a, P: AsRef<Path>> {
    data_dir: P,
    counter: AtomicPersistentUsize<'a>,
}

impl<'a, P: AsRef<Path>> ZippynfsServer<'a, P> {
    /// Returns a new ZippynfsServer
    pub fn new(data_dir: P) -> ZippynfsServer<'a, P> {
        // Read the fid counter
        let counter = AtomicPersistentUsize::from_file(&data_dir).unwrap();

        // Create the struct
        ZippynfsServer { data_dir, counter }
    }

    fn fs_find_by_fid(&self, fid: Fid) -> Result<Option<PathBuf>, String> {
        // Initialize state for BFS, starting at root
        let mut queue = VecDeque::new();
        queue.push_back((&self.data_dir).as_ref().to_path_buf());

        // Compile regex to check if a filename is a numbered file
        let re = Regex::new(r"^\d+$").unwrap();

        // For each iteration of BFS...
        while let Some(path) = queue.pop_front() {
            // If the numbered filename equals fid, return
            let cur: Fid = path.file_name().unwrap().to_str().unwrap().parse().unwrap();
            if cur == fid {
                return Ok(Some(path));
            }

            // If path is a dir...
            if path.is_dir() {
                // Expand path into it, or return with error
                let mut it = read_dir(path).map_err(|e| format!("{}", e))?;
                if it.any(|dirent| dirent.is_err()) {
                    return Err("Dirent is missing".into());
                }

                // Put numbered files (0/, 1/, 3, etc.) in numbered_files
                // Put named files (0.root, 1.foo, 3.zee.txt, etc.) in named_files
                let it = it.map(|dirent| dirent.unwrap().path());
                let (numbered_files, named_files): (HashSet<PathBuf>, _) = it.partition(|fname| {
                    re.is_match(fname.file_name().unwrap().to_str().unwrap())
                });

                // Extract fid's from named files into extracted_numbers
                let extracted_numbers = named_files
                    .iter()
                    .map(|fname| {
                        let named_file = fname
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .split(".")
                            .next()
                            .unwrap();
                        fname.parent().unwrap().join(named_file)
                    })
                    .collect();

                // Enqueue everything from set intersection of numbered_files and extracted_numbers
                // These represent NFS files for which both a numbered file and a named file exist
                let intersection = numbered_files.intersection(&extracted_numbers).cloned();
                queue.extend(intersection);
            }
        }

        // No such fid
        Ok(None)
    }
}

impl<'a, P: AsRef<Path>> ZippynfsSyncHandler for ZippynfsServer<'a, P> {
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
        //let parent = self.get_path(fsargs.where_.dir)?;

        // TODO: we ought to do something with inode/generation numbers here...

        // Create a new directory
        //let new_dir = format!("{}/{}", parent, fsargs.where_.filename);
        //create_dir(new_dir)?;

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
