
extern crate thrift;

mod counter;

#[cfg(test)]
mod test;

use std::fs::{create_dir, read_dir};
use std::path::{Path, PathBuf};
use std::collections::{HashSet, VecDeque};

use regex::Regex;

use self::counter::AtomicPersistentUsize;

use zippyrpc::*;

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
        let counter = AtomicPersistentUsize::from_file((data_dir).as_ref().join("counter"))
            .unwrap();

        // Create the struct
        ZippynfsServer { data_dir, counter }
    }

    /// A helper for `fs_find_by_fid`, which returns the named and numbered files in
    /// a given path, separating them by the given regex.
    fn get_numbered_and_named_files(
        &self,
        re: &Regex,
        path: &PathBuf,
    ) -> Result<(HashSet<PathBuf>, HashSet<PathBuf>), String> {
        // Expand path into it, or return with error
        assert!(path.exists());
        assert!(path.is_dir());
        let it = read_dir(path).map_err(|e| format!("{}", e))?;

        let mut path_bufs = Vec::new();
        for dirent in it {
            if dirent.is_err() {
                return Err("Dirent is missing".into());
            }
            path_bufs.push(dirent.unwrap().path());
        }
        let path_bufs = path_bufs;

        // Put numbered files (0/, 1/, 3, etc.) in numbered_files
        // Put named files (0.root, 1.foo, 3.zee.txt, etc.) in named_files
        Ok(path_bufs.into_iter().partition(|fname| {
            re.is_match(fname.file_name().unwrap().to_str().unwrap())
        }))
    }

    /// Returns the path to the file with the given `fid`.
    ///
    /// This is implemented as a BFS over the file system. We expect that it would be
    /// called very rarely, such as after a crash, once we have implemented caching.
    ///
    /// TODO: some sort of caching
    fn fs_find_by_fid(&self, fid: Fid) -> Result<Option<PathBuf>, String> {
        // Initialize state for BFS, starting at root
        let mut queue = VecDeque::new();
        queue.push_back((&self.data_dir).as_ref().join("0"));

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
                // Expand this node (dir) in the BFS
                let (numbered_files, named_files) = self.get_numbered_and_named_files(&re, &path)?;

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

    /// Get the id associated with a file named `fname` in the directory `path` on the NFS server.
    fn fs_find_by_name(&self, path: PathBuf, fname: &str) -> Result<Option<usize>, String> {
        // Sanity
        assert!(fname.len() > 0);

        // Compile regex to check if a filename is a numbered file
        let re_is_name = Regex::new(r"^\d+$").unwrap();

        // Compile regex to check if this is the file we want
        let re_is_file = Regex::new(&format!("^(\\d+)\\.{}$", fname)).unwrap();

        // Get the named and numbered files in the directory
        let (numbered_files, named_files) = self.get_numbered_and_named_files(&re_is_name, &path)?;

        for fname in named_files.iter() {
            // Found a match
            if let Some(id) = re_is_file
                .captures(fname.file_name().unwrap().to_str().unwrap())
                .map(|m| m.get(1).unwrap().as_str().parse().unwrap())
            {
                // Check that there is a matching numbered file
                if numbered_files.contains(&path.as_path().join(format!("{}", id))) {
                    return Ok(Some(id));
                }
            }
        }

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
        // Find the directory
        let dpath = self.fs_find_by_fid(fsargs.dir.fid as usize)?;

        // Make sure that directory exists
        if dpath.is_none() {
            // TODO: what NFS error do we return here?
            return Err("Unimplemented".into());
        }

        // Lookup the file in the directory
        let fid = self.fs_find_by_name(dpath.unwrap(), &fsargs.filename)?;

        // Return a result
        match fid {
            Some(fid) => {
                Ok(ZipDirOpRes::new(
                    ZipFileHandle::new(fid as i64),
                    ZipFattr::new(
                        ZipFtype::NFREG, // file type
                        0777, // mode
                        1, // number of links
                        0, // uid
                        0, // gid
                        0, // TODO size
                        0, // TODO blocksize
                        0, // rdev
                        0, // TODO blocks
                        0, // fsid
                        fid as i64,
                        ZipTimeVal::new(0, 0), // TODO: atime
                        ZipTimeVal::new(0, 0), // TODO: mtime
                        ZipTimeVal::new(0, 0), // TODO: ctime
                    ),
                ))
            }
            None => Err("No Such File or Directory".into()),
        }
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
