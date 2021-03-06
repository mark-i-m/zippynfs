
extern crate libc;
extern crate thrift;

mod counter;

#[cfg(test)]
mod test;

use std::cmp::min;
use std::fs::{create_dir, read_dir, remove_dir, remove_file, rename, copy, File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock, Arc};
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread::current;
use std::collections::{HashSet, HashMap, VecDeque};

use regex::Regex;

use zippyrpc::*;

use self::counter::AtomicPersistentUsize;

/// A type representing a File ID (FID)
type Fid = usize;

/// The size of FS block
const BLOCK_SIZE: u32 = 1 << 12; // 4KB

/// A regex for the filename format of the numbered server file for an NFS file
const NUMBERED_FILE_RE: &'static str = r"^\d+$";

/// The number of ns in a us
const NANOS_PER_MICRO: u32 = 1000;

/// Converts from `SystemTime` to `ZipTimeVal` used with Trift.
fn sys_time_to_zip_time(sys_time: SystemTime) -> ZipTimeVal {
    let since = sys_time.duration_since(UNIX_EPOCH).unwrap();

    let secs = since.as_secs();
    let nanos = since.subsec_nanos() / NANOS_PER_MICRO;

    ZipTimeVal::new(secs as i64, nanos as i64)
}

/// A server to handle RPC calls
pub struct ZippynfsServer<'a, P: AsRef<Path>> {
    /// The directory on the host system where the server stores stuff.
    data_dir: P,

    /// The unique fid generator
    counter: AtomicPersistentUsize<'a>,

    /// We need to be sure that no two files in the system have exactly the same path, so for the
    /// time until a file is created (or renamed) that name must be inserted into this set. The
    /// procedure is as follows (to insert a file called "foo" into directory with fid=3):
    ///
    /// 1. Grab the locked set
    /// 2. Insert /path/to/fs/1/3/foo to set
    /// 3. Release lock on set
    /// 4. Do FS stuff to create the file
    /// 5. Grab the locked set
    /// 6. Remove our entry from the set
    /// 7. Release the lock
    name_lock: Mutex<HashSet<(PathBuf, String)>>,

    /// The epoch number of this server. When it crashes, it should come up with a new number. This
    /// alerts writers that they probably should not count on cached data being there.
    ///
    /// In this implementation, we just use the next value of the FID counter, since it is unique.
    epoch: usize,

    /// A cache to map the FID of a file to the FID of its parent.
    fid_cache: RwLock<HashMap<Fid, Fid>>,

    /// Buffers for data written by the client asynchronously (with the UNSTABLE flag).
    ///
    /// Fid -> [(offset, size, data)]
    async_bufs: RwLock<HashMap<Fid, Arc<Mutex<Vec<(usize, usize, Vec<u8>)>>>>>,
}

impl<'a, P: AsRef<Path>> ZippynfsServer<'a, P> {
    /// Returns a new ZippynfsServer
    pub fn new(data_dir: P) -> ZippynfsServer<'a, P> {
        // Read the fid counter
        let counter = AtomicPersistentUsize::from_file((data_dir).as_ref().join("counter"))
            .unwrap();

        // Get the next FID to use as an epoch number for the server
        let epoch = counter.fetch_inc();

        // Create the struct
        ZippynfsServer {
            data_dir,
            counter,
            name_lock: Mutex::new(HashSet::new()),
            epoch,
            fid_cache: RwLock::new(HashMap::new()),
            async_bufs: RwLock::new(HashMap::new()),
        }
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

        // Put numbered files (1/, 3, etc.) in numbered_files
        // Put named files (1.root, 3.zee.txt, etc.) in named_files
        Ok(path_bufs.into_iter().partition(|fname| {
            re.is_match(fname.file_name().unwrap().to_str().unwrap())
        }))
    }

    /// Does most of the heavy lifting of `fs_find_by_fid` without looking in any cache.
    ///
    /// This is intended as a last resort, and we don't expect it to happen that often,
    /// except during a failover.
    ///
    /// If a path is found, it is returned, along with any mappings that should be inserted
    /// into the cache.
    fn fs_find_by_fid_no_cache(
        &self,
        fid: Fid,
    ) -> Result<Option<(PathBuf, Vec<(Fid, Fid)>)>, String> {
        // Initialize state for BFS, starting at root
        let mut queue = VecDeque::new();
        queue.push_back((&self.data_dir).as_ref().join("1"));

        // Compile regex to check if a filename is a numbered file
        let re = Regex::new(NUMBERED_FILE_RE).unwrap();

        // For each iteration of BFS...
        while let Some(path) = queue.pop_front() {
            // If the numbered filename equals fid, return
            let cur: Fid = path.file_name().unwrap().to_str().unwrap().parse().unwrap();
            if cur == fid {
                // Parse out the path to get a set of (file, parent) pairs which can be cached
                let heirarchy: Vec<_> = path.strip_prefix(&self.data_dir)
                    .unwrap()
                    .iter()
                    .map(|p| p.to_str().unwrap().parse().unwrap())
                    .collect();

                let mut pairs = Vec::new();

                for i in 0..(heirarchy.len() - 1) {
                    pairs.push((heirarchy[i + 1], heirarchy[i]));
                }

                return Ok(Some((path, pairs)));
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

    /// Does a series of reverse lookups in the `fid_cache` to trace a path from the FID
    /// back to the root.
    ///
    /// If a path is found, it is returned, along with any mappings that should be inserted
    /// into the cache.
    fn fs_find_by_fid_cached(
        &self,
        fid: Fid,
    ) -> Result<Option<(PathBuf, Vec<(Fid, Fid)>)>, String> {
        // Always know where the root is
        if fid == 1 {
            Ok(Some(((&self.data_dir).as_ref().join("1"), Vec::new())))
        } else {
            // Try to reverse-lookup a path all the way back to the root
            if let Some(parent_fid) = self.fid_cache.read().unwrap().get(&fid) {
                match self.fs_find_by_fid_cached(*parent_fid) {
                    Err(e) => Err(e),
                    Ok(None) => Ok(None),
                    Ok(Some((path, to_cache))) => {
                        Ok(Some((path.join(format!("{}", fid)), to_cache)))
                    }
                }
            } else {
                warn!("Required disk BFS for FID={}", fid);
                self.fs_find_by_fid_no_cache(fid)
            }
        }
    }

    /// Returns the path to the file with the given `fid`.
    ///
    /// This is implemented as a BFS over the file system. We expect that it would be
    /// called very rarely, such as after a crash, once we have implemented caching.
    fn fs_find_by_fid(&self, fid: Fid) -> Result<Option<PathBuf>, String> {
        // First, check the cache
        match self.fs_find_by_fid_cached(fid) {
            Err(e) => Err(e),
            Ok(None) => Ok(None),
            Ok(Some((path, to_cache))) => {
                // Insert any missing mappings into the cache
                self.fid_cache.write().unwrap().extend(to_cache);

                // Return the path
                Ok(Some(path))
            }
        }
    }

    /// Get the id associated with a file named `fname` in the directory `path` on the NFS server.
    fn fs_find_by_name(&self, path: PathBuf, fname: &str) -> Result<Option<usize>, String> {
        // Sanity
        assert!(fname.len() > 0);
        assert!(path.is_dir());

        // Compile regex to check if a filename is a numbered file
        let re_is_name = Regex::new(NUMBERED_FILE_RE).unwrap();

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

    /// Get the attributes of the given existing file.
    ///
    /// NOTE: This method ASSUMES the file actually exists! So you need to check before
    /// calling this method!
    fn fs_get_attr(&self, fpath_numbered: PathBuf, fid: u64) -> ZipFattr {
        // Sanity
        assert_eq!(
            fpath_numbered.file_name().unwrap().to_str().unwrap(),
            format!("{}", fid)
        );

        // Get attributes of the file
        let fmeta = fpath_numbered.metadata().unwrap();

        let size = fmeta.len() as u32;
        let blocks = (size + (BLOCK_SIZE - 1)) / BLOCK_SIZE;

        let created = if fmeta.created().is_ok() {
            sys_time_to_zip_time(fmeta.created().unwrap())
        } else {
            ZipTimeVal::new(0, 0)
        };
        let modified = if fmeta.modified().is_ok() {
            sys_time_to_zip_time(fmeta.modified().unwrap())
        } else {
            ZipTimeVal::new(0, 0)
        };
        let accessed = if fmeta.accessed().is_ok() {
            sys_time_to_zip_time(fmeta.accessed().unwrap())
        } else {
            ZipTimeVal::new(0, 0)
        };

        ZipFattr::new(
            if fpath_numbered.is_dir() {
                ZipFtype::NFDIR
            } else {
                ZipFtype::NFREG
            },
            0x1FF, // mode
            1, // number of links
            0, // uid
            0, // gid
            size as i64,
            BLOCK_SIZE as i64,
            0, // rdev
            blocks as i64,
            0, // fsid
            fid as i64,
            accessed,
            modified,
            created,
        )
    }

    /// Set the attributes on the given file.
    ///
    /// NOTE: For now, if you change `atime` you must also change `mtime`, and vice versa. If you
    /// only give `atime`, `mtime` is set to the same thing. If you only give `mtime` neither is
    /// set.
    ///
    /// NOTE: This method ASSUMES the file actually exists! So you need to check before calling
    /// this method!
    fn fs_set_attr(
        &self,
        fpath_numbered: PathBuf,
        _fid: Fid,
        atime: Option<ZipTimeVal>,
        mtime: Option<ZipTimeVal>,
        size: Option<usize>,
    ) -> thrift::Result<()> {
        // For class to help with *const c_char
        use std::ffi::CStr;
        use std::os::unix::io::AsRawFd;

        // Create f, so we can change its metadata
        let mut open_options = OpenOptions::new();
        if fpath_numbered.is_dir() {
            open_options.read(true)
        } else {
            open_options.read(true).write(true)
        };
        let f = open_options.open(&fpath_numbered).unwrap();

        // Update size
        if let Some(size) = size {
            if fpath_numbered.is_dir() {
                return Err(nfs_error(ZipErrorType::NFSERR_ISDIR));
            }
            f.set_len(size as u64).unwrap();
        }

        // Update accessed and modified time
        if let Some(atime) = atime {
            let mtime = if let Some(mtime) = mtime {
                mtime
            } else {
                atime.clone()
            };

            // Construct timespecs_ptr
            let access_timespec = libc::timespec {
                tv_sec: atime.seconds,
                tv_nsec: atime.useconds * (NANOS_PER_MICRO as i64),
            };
            let modified_timespec = libc::timespec {
                tv_sec: mtime.seconds,
                tv_nsec: mtime.useconds * (NANOS_PER_MICRO as i64),
            };
            let timespecs = [access_timespec, modified_timespec];
            let timespecs_ptr = &timespecs as *const libc::timespec;

            // Get raw fd
            let fd = f.as_raw_fd();

            // Call futimens()
            let retval = unsafe { libc::futimens(fd, timespecs_ptr) };

            if retval == -1 {
                let errno = unsafe { *libc::__errno_location() };
                if errno == libc::ENOENT {
                    // If the file was concurrently renamed or deleted, return stale
                    return Err(nfs_error(ZipErrorType::NFSERR_STALE));
                } else {
                    let errmsg_cstr = unsafe { CStr::from_ptr(libc::strerror(errno)) };
                    let errmsg = errmsg_cstr.to_str().unwrap();
                    panic!("Errno = {}, message: {}", errno, errmsg);
                }
            }
        }

        // There is no "sync_metadata()", so we call sync_all()
        f.sync_all().unwrap();

        Ok(())
    }

    /// Add the given name to the `name_lock`.
    ///
    /// Returns true if the name was locked and false it was already locked.
    fn lock_name(&self, name: (PathBuf, String)) -> bool {
        self.name_lock.lock().unwrap().insert(name)
    }

    /// Remove the given name from the `name_lock`
    ///
    /// This should always succeed.
    ///
    /// NOTE: The burden is on the caller to ensure the name is already in the `name_lock`.
    /// We will `panic!` otherwise!
    fn unlock_name(&self, name: &(PathBuf, String)) {
        let present = self.name_lock.lock().unwrap().remove(name);
        assert!(present);
    }

    /// Create the filesystem object in the given directory and increment counter
    ///
    /// NOTE: This method ASSUMES the object does not exist! So you need to check before
    /// calling this method!
    fn fs_create_obj(
        &self,
        dpath: PathBuf,
        fname: &str,
        is_file: bool,
    ) -> Result<(Fid, PathBuf), String> {
        let fid = self.counter.fetch_inc();
        let fpath_numbered = dpath.join(fid.to_string());
        let fpath_named = dpath.join(format!("{}.{}", fid, fname));

        // Create numbered file or directory
        if is_file {
            // NOTE: Because we don't implement permissions, set them to 600, so that the server can do
            // whatever it wants.
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&fpath_numbered)
                .map_err(|e| e.to_string())?;
        } else {
            create_dir(&fpath_numbered).map_err(|e| format!("{}", e))?;
        }

        // Sync the directory
        let dir = File::open(dpath).unwrap();
        dir.sync_all().map_err(|e| format!("{}", e))?;

        // Create named file
        File::create(&fpath_named).map_err(|e| format!("{}", e))?;

        // Sync the directory
        dir.sync_all().map_err(|e| format!("{}", e))?;

        // Done
        Ok((fid, fpath_numbered))
    }

    /// A helper for `handle_mkdir` and `handle_create`, which creates either a file
    /// or a directory depending on is_file.
    fn create_object(&self, fsargs: ZipCreateArgs, is_file: bool) -> thrift::Result<ZipDirOpRes> {
        // Find the directory
        let dpath = self.fs_find_by_fid(fsargs.where_.dir.fid as usize)?;
        debug!("Found parent at path {:?}", dpath);

        // Make sure that directory exists
        if dpath.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let dpath = dpath.unwrap();
        let filename = &fsargs.where_.filename;

        // Make sure dpath is a directory
        if !dpath.is_dir() {
            return Err(nfs_error(ZipErrorType::NFSERR_NOTDIR));
        }

        // Lock the name so that after we check we know we have the name
        if !self.lock_name((dpath.clone(), filename.clone())) {
            // Could not lock == name already exists (so one else got there first)
            return Err(nfs_error(ZipErrorType::NFSERR_EXIST));
        }

        // NOTE: We cannot use `?` until we unlock so as not to cause deadlock!

        // Make sure the given filename does not exist already
        let already = self.fs_find_by_name(dpath.clone(), &filename);

        // If we have some random error, then unlock
        if already.is_err() {
            self.unlock_name(&(dpath.clone(), filename.clone()));
            debug!("File \"{}\" exists", filename);
            return Err(already.err().unwrap().into());
        }

        // If the name already exists, then unlock
        if already.ok().unwrap().is_some() {
            self.unlock_name(&(dpath.clone(), filename.clone()));
            debug!("File \"{}\" exists", filename);
            return Err(nfs_error(ZipErrorType::NFSERR_EXIST));
        }

        // If we get to this point, we know that we own the name!

        // Create a new object
        let (new_fid, fpath_numbered) = self.fs_create_obj(dpath.clone(), filename, is_file)?;

        // Set attributes on the new file
        self.fs_set_attr(
            fpath_numbered.clone(),
            new_fid,
            fsargs.attributes.atime,
            fsargs.attributes.mtime,
            fsargs.attributes.size.map(|s| s as usize),
        )?;

        // Unlock filename
        self.unlock_name(&(dpath, filename.to_owned()));

        // Insert into cache
        self.fid_cache.write().unwrap().insert(
            new_fid,
            fsargs.where_.dir.fid as
                usize,
        );

        Ok(ZipDirOpRes::new(
            ZipFileHandle::new(new_fid as i64),
            self.fs_get_attr(fpath_numbered, new_fid as u64),
        ))
    }

    /// Delete the filesystem object in the given directory with the given fid
    ///
    /// NOTE: This method ASSUMES the file actually exists! So you need to check before
    /// calling this method!
    fn fs_delete_obj(
        &self,
        dpath: PathBuf,
        fid: u64,
        fname: &str,
        is_file: bool,
    ) -> Result<(), thrift::Error> {
        // Get the path of the file itself
        let fpath_numbered = dpath.join(format!("{}", fid));
        let fpath_named = dpath.join(format!("{}.{}", fid, fname));

        // Remove named file
        if is_file {
            remove_file(fpath_numbered).map_err(|e| format!("{}", e))?;
        } else {
            // The directory must be empty, so if we can get any dir entries,
            // return an error.
            if fpath_numbered.read_dir().unwrap().next().is_some() {
                return Err(nfs_error(ZipErrorType::NFSERR_NOTEMPTY));
            }

            remove_dir(fpath_numbered).map_err(|e| format!("{}", e))?;
        }

        // Lock the `fid_cache` while we remove
        let mut fid_cache_locked = self.fid_cache.write().unwrap();

        // Remove the fid from the cache
        let _ = fid_cache_locked.remove(&(fid as usize));

        // Sync the directory
        let dir = File::open(dpath).unwrap();
        dir.sync_all().map_err(|e| format!("{}", e))?;

        // Remove numbered file
        remove_file(fpath_named).map_err(|e| format!("{}", e))?;

        // Sync the directory
        dir.sync_all().map_err(|e| format!("{}", e))?;

        // `fid_cache_locked` dropped

        // Done
        Ok(())
    }

    /// Get a set of `(fid, name)` for all entries in the given directory.
    fn fs_read_dir(&self, dpath: PathBuf) -> Result<HashSet<(u64, String, ZipFtype)>, String> {
        let re = Regex::new(NUMBERED_FILE_RE).unwrap();
        let (numbered_files, named_files) = self.get_numbered_and_named_files(&re, &dpath)?;

        Ok(
            named_files
                .into_iter()
                .map(|fname| {
                    let mut named_file =
                        fname.file_name().unwrap().to_str().unwrap().splitn(2, ".");
                    let number = named_file.next().unwrap();
                    let name = named_file.next().unwrap();

                    let numbered_file = fname.parent().unwrap().join(number);

                    let ftype = if numbered_file.is_dir() {
                        ZipFtype::NFDIR
                    } else {
                        ZipFtype::NFREG
                    };

                    (numbered_file, (
                        number.parse().unwrap(),
                        name.to_owned(),
                        ftype,
                    ))
                })
                .filter(|&(ref numbered_file, _)| {
                    numbered_files.contains(numbered_file)
                })
                .map(|(_, pair)| pair)
                .collect(),
        )
    }

    fn fs_stable_write(
        &self,
        fid: Fid,
        offset: usize,
        count: usize,
        buf: &[u8],
    ) -> thrift::Result<usize> {
        // find the file
        let fpath_numbered = self.fs_find_by_fid(fid)?;
        debug!("Found file at path {:?}", fpath_numbered);

        // Make sure it exists
        if fpath_numbered.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let fpath_numbered = fpath_numbered.unwrap();

        // Create a tmp file by copying the existing file
        //
        // We name the tmp file after the FID and this thread's TID so
        // as to avoid interleaving writes from different client reqs.
        let tid = current().id();
        let tmp_fpath = (&self.data_dir).as_ref().join(
            format!("tmp/{}_{:?}", fid, tid),
        );
        copy(&fpath_numbered, &tmp_fpath)?;

        {
            // Open the file for the write
            let mut tmp_file = OpenOptions::new().write(true).open(&tmp_fpath)?;

            // Sync the tmp file to ensure we have its contents
            tmp_file.sync_all()?;

            // Seek to the write location
            tmp_file.seek(SeekFrom::Start(offset as u64))?;

            // Write the data to the file
            assert_eq!(buf.len(), count);
            tmp_file.write_all(buf)?;

            // Sync the file
            tmp_file.sync_all()?;
        } // File closed

        // Atomic rename file
        rename(tmp_fpath, fpath_numbered)?;

        Ok(buf.len())
    }

    /// Get or create the `async_bufs` entry for the given FID.
    fn get_or_create_async_bufs(&self, fid: Fid) -> Arc<Mutex<Vec<(usize, usize, Vec<u8>)>>> {
        // Check if the entry exists in the table already
        let try_read = {
            self.async_bufs.read().unwrap().get(&fid).map(|e| e.clone())
        }; // LOCK DROPPED (otherwise, the write lock acquire might deadlock)

        if let Some(entry) = try_read {
            debug!("Read FID entry");
            entry.clone()
        } else {
            debug!("Created FID entry");
            // Writer lock, then insert new entry
            let mut write_locked = self.async_bufs.write().unwrap();

            debug!("Write lock on async table");
            write_locked.insert(fid, Arc::new(Mutex::new(Vec::new())));

            // Get the entry and return
            write_locked.get(&fid).unwrap().clone()
        }
    }
}

impl<'a, P: AsRef<Path>> ZippynfsSyncHandler for ZippynfsServer<'a, P> {
    fn handle_null(&self) -> thrift::Result<i64> {
        info!("Handling NULL");
        Ok(self.epoch as i64)
    }

    fn handle_getattr(&self, fhandle: ZipFileHandle) -> thrift::Result<ZipAttrStat> {
        info!("Handling GETATTR {:?}", fhandle);

        // Find the file/directory
        let fpath_numbered = self.fs_find_by_fid(fhandle.fid as usize)?;

        // Return a result
        match fpath_numbered {
            Some(fpath_numbered) => {
                debug!("Found file at server path {:?}", fpath_numbered);
                Ok(ZipAttrStat::new(
                    self.fs_get_attr(fpath_numbered, fhandle.fid as u64),
                ))
            }
            None => {
                debug!("No such file with fid = {}", fhandle.fid);
                Err(nfs_error(ZipErrorType::NFSERR_STALE))
            }
        }
    }

    fn handle_setattr(&self, fsargs: ZipSattrArgs) -> thrift::Result<ZipAttrStat> {
        info!("Handling SETATTR {:?}", fsargs);

        // Find the file/directory. Note that the file may be concurrently renamed or deleted
        // between here and the call to the libc function.
        let fpath_numbered = self.fs_find_by_fid(fsargs.file.fid as usize)?;

        // Return a result
        match fpath_numbered {
            Some(fpath_numbered) => {
                debug!("Found file at server path {:?}", fpath_numbered);

                // Attempt to set attributes
                self.fs_set_attr(
                    fpath_numbered.clone(),
                    fsargs.file.fid as Fid,
                    fsargs.attributes.atime,
                    fsargs.attributes.mtime,
                    fsargs.attributes.size.map(|s| s as usize),
                )?;

                // Done
                Ok(ZipAttrStat::new(
                    self.fs_get_attr(fpath_numbered, fsargs.file.fid as u64),
                ))
            }
            None => {
                debug!("No such file with fid = {}", fsargs.file.fid);
                Err(nfs_error(ZipErrorType::NFSERR_STALE))
            }
        }
    }

    fn handle_lookup(&self, fsargs: ZipDirOpArgs) -> thrift::Result<ZipDirOpRes> {
        info!("Handling LOOKUP {:?}", fsargs);

        // Find the directory
        let dpath = self.fs_find_by_fid(fsargs.dir.fid as usize)?;
        debug!("Found parent at path {:?}", dpath);

        // Make sure that directory exists
        if dpath.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let dpath = dpath.unwrap();

        // Make sure dpath is a directory
        if !dpath.is_dir() {
            return Err(nfs_error(ZipErrorType::NFSERR_NOTDIR));
        }

        // Lookup the file in the directory
        let fid = self.fs_find_by_name(dpath.clone(), &fsargs.filename)?;

        // Return a result
        match fid {
            Some(fid) => {
                debug!("File \"{}\" with fid = {}", fsargs.filename, fid);

                // Get attributes of the file
                let fpath_numbered = dpath.join(format!("{}", fid));

                Ok(ZipDirOpRes::new(
                    ZipFileHandle::new(fid as i64),
                    self.fs_get_attr(fpath_numbered, fid as u64),
                ))
            }
            None => {
                debug!("File \"{}\" does not exist", fsargs.filename);
                Err(nfs_error(ZipErrorType::NFSERR_NOENT))
            }
        }
    }

    fn handle_read(&self, fsargs: ZipReadArgs) -> thrift::Result<ZipReadRes> {
        // For File::read_at() on Unix-like systems
        use std::os::unix::fs::FileExt;

        info!("Handling READ {:?}", fsargs);

        // Find the file
        let fpath_numbered = self.fs_find_by_fid(fsargs.file.fid as usize)?;
        debug!("Found file at path {:?}", fpath_numbered);

        // Make sure that file exists
        if fpath_numbered.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let fpath_numbered = fpath_numbered.unwrap();

        // Make sure fpath_numbered is not a directory
        if !fpath_numbered.is_file() {
            return Err(nfs_error(ZipErrorType::NFSERR_ISDIR));
        }

        // Get file contents
        let mut data = vec![0; min(fsargs.count as usize, MAX_BUF_LEN)];
        {
            let f = File::open(&fpath_numbered).unwrap();
            // The underlying filesystem makes sure this works, even if another thread
            // concurrently renames or unlinks the file.
            let actual_size = f.read_at(&mut data[..], fsargs.offset as u64).unwrap();
            data.resize(actual_size, 0);
        }
        let data = data;
        //debug!("Contents: {:?}", data);
        debug!("Contents Length: {:?}", data.len());

        // Done
        Ok(ZipReadRes::new(
            self.fs_get_attr(fpath_numbered, fsargs.file.fid as u64),
            data,
        ))
    }

    fn handle_write(&self, fsargs: ZipWriteArgs) -> thrift::Result<ZipWriteRes> {
        info!(
            "Handling WRITE fid={}, offset={}, count={}, stable={:?}",
            fsargs.file.fid,
            fsargs.offset,
            fsargs.count,
            fsargs.stable
        );
        debug!("{}", String::from_utf8_lossy(&fsargs.data));

        match fsargs.stable {
            ZipWriteStable::FILE_SYNC |
            ZipWriteStable::DATA_SYNC => {
                // Do a stable write
                let bytes = self.fs_stable_write(
                    fsargs.file.fid as usize,
                    fsargs.offset as usize,
                    fsargs.count as usize,
                    &fsargs.data,
                )?;

                // Sanity
                assert_eq!(bytes, fsargs.data.len());

                // DONE!
                Ok(ZipWriteRes::new(
                    bytes as i64,
                    ZipWriteStable::FILE_SYNC,
                    self.epoch as i64,
                ))
            }

            ZipWriteStable::UNSTABLE => {
                let size = fsargs.count as usize;

                // Sanity
                assert_eq!(fsargs.data.len(), size);

                // Get or create the appropriate buffer set and append the given data to the buffer
                let unlocked = self.get_or_create_async_bufs(fsargs.file.fid as Fid);

                debug!("Got unlocked FID entry");
                unlocked.lock().unwrap().push((
                    fsargs.offset as usize,
                    size,
                    fsargs.data,
                ));

                // Immediately ACK
                Ok(ZipWriteRes::new(
                    size as i64,
                    ZipWriteStable::UNSTABLE,
                    self.epoch as i64,
                ))
            }
        }
    }

    fn handle_create(&self, fsargs: ZipCreateArgs) -> thrift::Result<ZipDirOpRes> {
        info!("Handling CREATE {:?}", fsargs);

        self.create_object(fsargs, true)
    }

    fn handle_remove(&self, fsargs: ZipDirOpArgs) -> thrift::Result<()> {
        info!("Handling REMOVE {:?}", fsargs);

        // find the directory
        let dpath = self.fs_find_by_fid(fsargs.dir.fid as usize)?;
        debug!("Found parent at path {:?}", dpath);

        // make sure that directory exists
        if dpath.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let dpath = dpath.unwrap();

        // make sure dpath is a directory
        if !dpath.is_dir() {
            return Err(nfs_error(ZipErrorType::NFSERR_NOTDIR));
        }

        // lookup the file in the directory
        let fid = self.fs_find_by_name(dpath.clone(), &fsargs.filename)?;

        match fid {
            Some(fid) => {
                debug!("File \"{}\" with fid = {}", fsargs.filename, fid);

                // should make sure that it is a file
                if dpath.join(format!("{}", fid)).is_dir() {
                    Err(nfs_error(ZipErrorType::NFSERR_ISDIR))
                } else {
                    // Remove the object
                    self.fs_delete_obj(
                        dpath,
                        fid as u64,
                        &fsargs.filename,
                        true,
                    )?;
                    Ok(())
                }
            }
            None => {
                debug!("File \"{}\" does not exist", fsargs.filename);
                Err(nfs_error(ZipErrorType::NFSERR_NOENT))
            }
        }
    }

    fn handle_rename(&self, fsargs: ZipRenameArgs) -> thrift::Result<()> {
        info!("Handling RENAME {:?}", fsargs);

        // Find the old directory
        let old_loc_dpath = self.fs_find_by_fid(fsargs.old_loc.dir.fid as usize)?;

        // Make sure it exists
        let old_loc_dpath = if let Some(path) = old_loc_dpath {
            path
        } else {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        };

        // Find the new directory
        let new_loc_dpath = self.fs_find_by_fid(fsargs.new_loc.dir.fid as usize)?;

        // Make sure it exists
        let new_loc_dpath = if let Some(path) = new_loc_dpath {
            path
        } else {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        };

        // Find the file to be moved
        let fid = self.fs_find_by_name(
            old_loc_dpath.clone(),
            &fsargs.old_loc.filename,
        )?;

        // make sure it exists
        if fid.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_NOENT));
        }

        // Lock the name so that after we check we know we have the name
        if !self.lock_name((new_loc_dpath.clone(), fsargs.new_loc.filename.clone())) {
            // Could not lock == name already exists (so one else got there first)
            return Err(nfs_error(ZipErrorType::NFSERR_EXIST));
        }

        // NOTE: We cannot use `?` until we unlock so as not to cause deadlock!

        // Make sure the given filename does not exist already
        let already = self.fs_find_by_name(new_loc_dpath.clone(), &fsargs.new_loc.filename);

        // If we have some random error, then unlock
        if already.is_err() {
            self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename.clone()));
            return Err(already.err().unwrap().into());
        }

        // If the name already exists, then unlock
        if already.ok().unwrap().is_some() {
            self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename.clone()));
            return Err(nfs_error(ZipErrorType::NFSERR_EXIST));
        }

        // If we get to this point, we know that we own the name!

        let new_loc_dir = File::open(new_loc_dpath.clone()).unwrap();

        let fid = fid.unwrap();
        let old_loc_fpath_named =
            old_loc_dpath.join(format!("{}.{}", fid, fsargs.old_loc.filename));
        let new_loc_fpath_named = new_loc_dpath.clone().join(format!(
            "{}.{}",
            fid,
            fsargs.new_loc.filename.clone()
        ));

        // Create the new named file
        let res = File::create(&new_loc_fpath_named);
        if res.is_err() {
            self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename.clone()));
            return Err(res.err().unwrap().into());
        }

        // Sync the directory
        let res = new_loc_dir.sync_all();
        if res.is_err() {
            self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename.clone()));
            return Err(res.err().unwrap().into());
        }

        // Atomic rename numbered file to new location
        //
        // While we are doing the rename itself, we need to keep the `fid_cache` locked
        let old_loc_fpath_numbered = old_loc_dpath.join(fid.to_string());
        let new_loc_fpath_numbered = new_loc_dpath.clone().join(fid.to_string());
        {
            let mut fid_cache_locked = self.fid_cache.write().unwrap();

            let res = rename(old_loc_fpath_numbered, new_loc_fpath_numbered);
            if res.is_err() {
                self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename.clone()));
                return Err(res.err().unwrap().into());
            }

            // Sync the directory
            let res = new_loc_dir.sync_all();
            if res.is_err() {
                self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename));
                return Err(res.err().unwrap().into());
            }

            // Update the cache if the value is in it. Otherwise insert it.
            let old = fid_cache_locked.insert(fid, fsargs.new_loc.dir.fid as usize);

            // Sanity
            if let Some(old) = old {
                assert_eq!(old, fsargs.old_loc.dir.fid as usize);
            }
        } // unlock `fid_cache`

        // At this point the file has been renamed... we just need to clean up

        // Unlock the name
        self.unlock_name(&(new_loc_dpath.clone(), fsargs.new_loc.filename));

        // Remove the old named file... we don't even need to sync!
        remove_file(old_loc_fpath_named)?;

        // DONE!
        Ok(())
    }

    fn handle_mkdir(&self, fsargs: ZipCreateArgs) -> thrift::Result<ZipDirOpRes> {
        info!("Handling MKDIR {:?}", fsargs);

        self.create_object(fsargs, false)
    }

    fn handle_rmdir(&self, fsargs: ZipDirOpArgs) -> thrift::Result<()> {
        info!("Handling RMDIR {:?}", fsargs);

        // Find the directory
        let dpath = self.fs_find_by_fid(fsargs.dir.fid as usize)?;
        debug!("Found parent at path {:?}", dpath);

        // Make sure that directory exists
        if dpath.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let dpath = dpath.unwrap();

        // Make sure dpath is a directory
        if !dpath.is_dir() {
            return Err(nfs_error(ZipErrorType::NFSERR_NOTDIR));
        }

        // Lookup the file in the directory
        let fid = self.fs_find_by_name(dpath.clone(), &fsargs.filename)?;

        match fid {
            Some(fid) => {
                debug!("File \"{}\" with fid = {}", fsargs.filename, fid);

                // should make sure that it is a dir
                if !dpath.join(format!("{}", fid)).is_dir() {
                    Err(nfs_error(ZipErrorType::NFSERR_NOTDIR))
                } else {
                    // Remove the object
                    self.fs_delete_obj(
                        dpath,
                        fid as u64,
                        &fsargs.filename,
                        false,
                    )?;
                    Ok(())
                }
            }
            None => {
                debug!("File \"{}\" does not exist", fsargs.filename);
                Err(nfs_error(ZipErrorType::NFSERR_NOENT))
            }
        }
    }

    fn handle_readdir(&self, fsargs: ZipReadDirArgs) -> thrift::Result<ZipReadDirRes> {
        info!("Handling READDIR {:?}", fsargs);

        // Find the directory
        let dpath = self.fs_find_by_fid(fsargs.dir.fid as usize)?;
        debug!("Found parent at path {:?}", dpath);

        // Make sure that directory exists
        if dpath.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let dpath = dpath.unwrap();

        // Make sure dpath is a directory
        if !dpath.is_dir() {
            return Err(nfs_error(ZipErrorType::NFSERR_NOTDIR));
        }

        // Get directory contents
        let contents = self.fs_read_dir(dpath)?;

        if contents.len() <= (fsargs.offset as usize) {
            debug!("END OF DIR");

            // No more entries
            Ok(ZipReadDirRes::new(Vec::new()))
        } else {
            // Collect the contents into an ordered collection
            let mut contents: Vec<_> = contents
                .into_iter()
                .map(|(fid, fname, ftype)| {
                    ZipDirEntry::new(fid as i64, fname, ftype)
                })
                .collect();

            // Sort
            contents.sort_unstable_by(|de1, de2| de1.fid.cmp(&de2.fid));

            // Get just the offset we want.
            //
            // Note that we cannot return more than MAX_BUF_LEN bytes
            let num_de = min(MAX_BUF_LEN / mem::size_of::<ZipDirEntry>(), contents.len());

            let mut contents = contents.split_off(fsargs.offset as usize);
            contents.truncate(num_de);

            debug!("Returning {} dirents", contents.len());
            debug!("Contents: {:?}", contents);

            Ok(ZipReadDirRes::new(contents))
        }
    }

    fn handle_statfs(&self, _: ZipFileHandle) -> thrift::Result<ZipStatFsRes> {
        info!("Handling STATFS");

        // I totally made up these numbers... maybe they are reasonable (but probably not)
        Ok(ZipStatFsRes::new(
            2 << 20,
            1 << 12,
            1 << 20,
            1 << 20,
            1 << 20,
        ))
    }

    fn handle_commit(&self, fsargs: ZipCommitArgs) -> thrift::Result<ZipCommitRes> {
        info!("Handling COMMMIT {:?}", fsargs);

        // find the file
        let fpath_numbered = self.fs_find_by_fid(fsargs.file.fid as Fid)?;
        debug!("Found file at path {:?}", fpath_numbered);

        // Make sure it exists
        if fpath_numbered.is_none() {
            return Err(nfs_error(ZipErrorType::NFSERR_STALE));
        }

        let fpath_numbered = fpath_numbered.unwrap();

        // Grab that set of changes out of the table
        let to_write = self.async_bufs.write().unwrap().remove(
            &(fsargs.file.fid as Fid),
        );

        // If there are no changes to be committed, then return success immediately
        if to_write.is_none() {
            return Ok(ZipCommitRes::new(self.epoch as i64));
        }

        // Extract from the mutex since we alone have this entry
        //
        // NOTE: we still need to lock and empty this Vec because somebody else could be holding a
        // copy of the Arc and waiting for the lock. We need to make sure that they don't try to do
        // the same work we just did.
        let unlocked = to_write.unwrap();
        let mut to_write = unlocked.lock().unwrap();

        // If there are no changes to be committed, then return success immediately
        if to_write.is_empty() {
            return Ok(ZipCommitRes::new(self.epoch as i64));
        }

        // Ok, so at this point we know that there is work to do, so let's do it!

        // Create a tmp file by copying the existing file
        //
        // We name the tmp file after the FID and this thread's TID so
        // as to avoid interleaving writes from different client reqs.
        let tid = current().id();
        let tmp_fpath = (&self.data_dir).as_ref().join(format!(
            "tmp/{}_{:?}",
            fsargs.file.fid,
            tid
        ));
        copy(&fpath_numbered, &tmp_fpath)?;

        {
            // Open the file for the write
            let mut tmp_file = OpenOptions::new().write(true).open(&tmp_fpath)?;

            // Sync the tmp file to ensure we have its contents
            tmp_file.sync_all()?;

            // Do each write onto the tmp file and sync them together
            //
            // NOTE: this also empties the Vec, so that future lockers will see no work to do!
            for (offset, count, buf) in to_write.drain(..) {
                // Seek to the write location
                tmp_file.seek(SeekFrom::Start(offset as u64))?;

                // Write the data to the file
                assert_eq!(buf.len(), count);
                tmp_file.write_all(&buf)?;
            }

            // Sync the file
            tmp_file.sync_all()?;
        } // File closed

        // Atomic rename file
        rename(tmp_fpath, fpath_numbered)?;

        Ok(ZipCommitRes::new(self.epoch as i64))
    }
}
