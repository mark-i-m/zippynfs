//! Unit tests for ZippynfsServer

use std::collections::HashSet;
use std::process::Command;
#[allow(unused_imports)]
use std::error::Error as std_err;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use regex::Regex;

use zippyrpc::*;

use super::AtomicPersistentUsize;
use super::ZippynfsServer;

/// Prevent multiple concurrent test from running at the same time
/// because we open too many file descriptors.
lazy_static! {
    static ref CONC_TEST_LOCK: Mutex<()> = Mutex::new(());
}

fn cleanup_git_hackery_test1<P>(fspath: P)
where
    P: AsRef<Path>,
{
    use std::fs::remove_file;

    remove_file((&fspath).as_ref().join("0/5/32.empty")).unwrap();
    remove_file((&fspath).as_ref().join("0/6/33.empty")).unwrap();
    remove_file((&fspath).as_ref().join("tmp/0")).unwrap();
}

fn run_with_clone_fs<'t, P, F>(fspath: P, clean_after: bool, f: F)
where
    P: AsRef<Path>,
    F: FnOnce(&Path) -> (),
{
    use std::fs::remove_dir_all;

    lazy_static! {
        static ref FSCOUNT: AtomicUsize = AtomicUsize::new(0);
    }

    // Create a new unique clone name
    let new_clone: PathBuf = (&format!(
        "test_files/_clone_fs_{}",
        FSCOUNT.fetch_add(1, Ordering::SeqCst)
    )).into();

    // Cleanup after previous attempts
    if new_clone.exists() {
        remove_dir_all(&new_clone).unwrap();
    }

    // Create the new clone
    assert!{
        Command::new("cp")
            .args(&["-r", (&fspath).as_ref().to_str().unwrap(), new_clone.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    };

    // Run the user's test
    f(&new_clone);

    // Cleanup afterwards, if needed
    if clean_after {
        remove_dir_all(new_clone).unwrap();
    }
}

fn fake_dir_op_args(did: i64, filename: &str) -> ZipDirOpArgs {
    ZipDirOpArgs::new(ZipFileHandle::new(did), filename.to_owned())
}

fn fake_read_args(fid: i64, offset: i64, count: i64) -> ZipReadArgs {
    ZipReadArgs::new(ZipFileHandle::new(fid), offset, count)
}

fn fake_create_args(did: i64, filename: &str) -> ZipCreateArgs {
    let where_ = fake_dir_op_args(did, &filename);
    let attributes = ZipSattr::new(0, 0, 0, ZipTimeVal::new(0, 0), ZipTimeVal::new(0, 0));
    ZipCreateArgs::new(where_, attributes)
}

fn fake_rename_args(
    old_did: i64,
    old_filename: &str,
    new_did: i64,
    new_filename: &str,
) -> ZipRenameArgs {
    let old = fake_dir_op_args(old_did, &old_filename);
    let new = fake_dir_op_args(new_did, &new_filename);
    ZipRenameArgs::new(old, new)
}

#[test]
fn test_atomic_persistent_usize() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        {
            let counter = AtomicPersistentUsize::from_file(fspath.join("counter")).unwrap();

            assert_eq!(counter.fetch_inc(), 8);
            assert_eq!(counter.fetch_inc(), 9);
            assert_eq!(counter.fetch_inc(), 10);
            assert_eq!(counter.fetch_inc(), 11);
            assert_eq!(counter.fetch_inc(), 12);
        } // Close file

        {
            let counter = AtomicPersistentUsize::from_file(fspath.join("counter")).unwrap();

            assert_eq!(counter.fetch_inc(), 13);
            assert_eq!(counter.fetch_inc(), 14);
            assert_eq!(counter.fetch_inc(), 15);
            assert_eq!(counter.fetch_inc(), 16);
            assert_eq!(counter.fetch_inc(), 17);
        } // Close file
    })
}

#[test]
fn test_atomic_persistent_usize_concurrent() {
    use std::sync::Arc;
    use std::thread;

    let _ctl = CONC_TEST_LOCK.lock();

    run_with_clone_fs("test_files/test1", true, |fspath| {
        const NTHREADS: usize = 1000;

        let counter = Arc::new(
            AtomicPersistentUsize::from_file(fspath.join("counter")).unwrap(),
        );
        let mut children = Vec::with_capacity(NTHREADS);
        let mut counts = [0xFFFF_FFFF_FFFF_FFFF; NTHREADS];

        // Create a bunch of racing threads
        for i in 0..NTHREADS {
            let counter = counter.clone();

            // We know that the indices are mutually exclusive, so this is not a race conditions.
            // We also know that the main thread will outlive all other threads.
            //
            // So this is safe.
            let count =
                unsafe { &mut *(counts.get_unchecked_mut(i) as *const usize as *mut usize) };

            children.push(thread::spawn(move || { *count = counter.fetch_inc(); }));
        }

        // Wait for all threads to exit
        for child in children {
            child.join().unwrap();
        }

        // Correctness

        // Last value is written
        assert_eq!(counter.fetch_inc(), 1008);

        // Sort values each thread got
        counts.sort();

        // All children got unique values
        for i in 0..NTHREADS {
            assert_eq!(counts[i], i + 8);
        }
    })
}

#[test]
fn test_new() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Make sure we can create a server
        let _server = ZippynfsServer::new(fspath);
    })
}

#[test]
fn test_get_numbered_and_named_files() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        let re = Regex::new(r"^\d+$").unwrap();
        let path = fspath.join("0");
        let (numbered_files, named_files) =
            server.get_numbered_and_named_files(&re, &path).unwrap();
        assert_eq!(numbered_files.len(), 4);
        assert_eq!(named_files.len(), 4);

        assert!(numbered_files.contains(&fspath.join("0/1")));
        assert!(numbered_files.contains(&fspath.join("0/4")));
        assert!(numbered_files.contains(&fspath.join("0/5")));
        assert!(numbered_files.contains(&fspath.join("0/6")));

        assert!(named_files.contains(&fspath.join("0/1.foo")));
        assert!(named_files.contains(&fspath.join("0/4.baz.txt")));
        assert!(named_files.contains(&fspath.join("0/5.bazee")));
        assert!(named_files.contains(&fspath.join("0/7.deleted.txt")));
    })
}

#[test]
fn test_fs_find_by_fid() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // then do a bunch of find_by_ids and verify the results
        let path0 = server.fs_find_by_fid(0);
        let path1 = server.fs_find_by_fid(1);
        let path2 = server.fs_find_by_fid(2);
        let path3 = server.fs_find_by_fid(3);
        let path4 = server.fs_find_by_fid(4);
        let path5 = server.fs_find_by_fid(5);
        let path6 = server.fs_find_by_fid(6);
        let path7 = server.fs_find_by_fid(7);

        // Correctness
        assert_eq!(path0, Ok(Some(fspath.join("0"))));
        assert_eq!(path1, Ok(Some(fspath.join("0/1"))));
        assert_eq!(path2, Ok(Some(fspath.join("0/1/2"))));
        assert_eq!(path3, Ok(Some(fspath.join("0/1/2/3"))));
        assert_eq!(path4, Ok(Some(fspath.join("0/4"))));
        assert_eq!(path5, Ok(Some(fspath.join("0/5"))));
        assert_eq!(path6, Ok(None));
        assert_eq!(path7, Ok(None));
    })
}

#[test]
fn test_fs_find_by_name() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Look for a bunch of stuff, and make sure we get the right results
        let find1 = server.fs_find_by_name(fspath.join("0"), "foo");
        let find2 = server.fs_find_by_name(fspath.join("0/1"), "bar");
        let find3 = server.fs_find_by_name(fspath.join("0/1/2"), "zee.txt");
        let find4 = server.fs_find_by_name(fspath.join("0"), "baz.txt");
        let find5 = server.fs_find_by_name(fspath.join("0"), "bazee");
        let find7 = server.fs_find_by_name(fspath.join("0"), "deleted.txt");
        let find8 = server.fs_find_by_name(fspath.join("0"), ".");
        let find9 = server.fs_find_by_name(fspath.join("0"), "fignewton");

        // Correctness
        assert_eq!(find1, Ok(Some(1)));
        assert_eq!(find2, Ok(Some(2)));
        assert_eq!(find3, Ok(Some(3)));
        assert_eq!(find4, Ok(Some(4)));
        assert_eq!(find5, Ok(Some(5)));
        assert_eq!(find7, Ok(None));
        assert_eq!(find8, Ok(None));
        assert_eq!(find9, Ok(None));
    })
}

#[test]
fn test_fs_get_attr() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Get attributes for a bunch of files
        let attr0 = server.fs_get_attr(fspath.join("0"), 0);
        let attr1 = server.fs_get_attr(fspath.join("0/1"), 1);
        let attr2 = server.fs_get_attr(fspath.join("0/1/2"), 2);
        let attr3 = server.fs_get_attr(fspath.join("0/1/2/3"), 3);
        let attr4 = server.fs_get_attr(fspath.join("0/4"), 4);
        let attr5 = server.fs_get_attr(fspath.join("0/5"), 5);

        // Correctness
        assert_eq!(attr0.fid, 0);
        assert_eq!(attr1.fid, 1);
        assert_eq!(attr2.fid, 2);
        assert_eq!(attr3.fid, 3);
        assert_eq!(attr4.fid, 4);
        assert_eq!(attr5.fid, 5);

        // For files:
        assert_eq!(attr3.size, 27);
        assert_eq!(attr4.size, 0);

        assert_eq!(attr0.type_, ZipFtype::NFDIR);
        assert_eq!(attr1.type_, ZipFtype::NFDIR);
        assert_eq!(attr2.type_, ZipFtype::NFDIR);
        assert_eq!(attr3.type_, ZipFtype::NFREG);
        assert_eq!(attr4.type_, ZipFtype::NFREG);
        assert_eq!(attr5.type_, ZipFtype::NFDIR);
    })
}

#[test]
fn test_nfs_lookup() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // LOOKUP a bunch of things
        let lookup1 = server.handle_lookup(fake_dir_op_args(0, "foo")).unwrap();
        let lookup2 = server.handle_lookup(fake_dir_op_args(1, "bar")).unwrap();
        let lookup3 = server
            .handle_lookup(fake_dir_op_args(2, "zee.txt"))
            .unwrap();
        let lookup4 = server
            .handle_lookup(fake_dir_op_args(0, "baz.txt"))
            .unwrap();
        let lookup5 = server.handle_lookup(fake_dir_op_args(0, "bazee")).unwrap();
        let lookup7 = server.handle_lookup(fake_dir_op_args(0, "deleted.txt"));
        let lookup8 = server.handle_lookup(fake_dir_op_args(1, "foo"));

        // Correctness
        assert!(lookup7.is_err());
        match lookup7.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }

        assert!(lookup8.is_err());
        match lookup8.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }

        assert_eq!(lookup1.file.fid, 1);
        assert_eq!(lookup2.file.fid, 2);
        assert_eq!(lookup3.file.fid, 3);
        assert_eq!(lookup4.file.fid, 4);
        assert_eq!(lookup5.file.fid, 5);

        // For the two files
        assert_eq!(lookup3.attributes.size, 27);
        assert_eq!(lookup4.attributes.size, 0);

        assert_eq!(lookup1.attributes.fid, 1);
        assert_eq!(lookup2.attributes.fid, 2);
        assert_eq!(lookup3.attributes.fid, 3);
        assert_eq!(lookup4.attributes.fid, 4);
        assert_eq!(lookup5.attributes.fid, 5);

        assert_eq!(lookup1.attributes.type_, ZipFtype::NFDIR);
        assert_eq!(lookup2.attributes.type_, ZipFtype::NFDIR);
        assert_eq!(lookup3.attributes.type_, ZipFtype::NFREG);
        assert_eq!(lookup4.attributes.type_, ZipFtype::NFREG);
        assert_eq!(lookup5.attributes.type_, ZipFtype::NFDIR);
    })
}

#[test]
fn test_nfs_read() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // READ a bunch of things
        let read1 = server.handle_read(fake_read_args(3, 1, 10)).unwrap();
        let read2 = server.handle_read(fake_read_args(3, 0, 30)).unwrap();
        let read3 = server.handle_read(fake_read_args(3, 30, 10)).unwrap();
        let read4 = server.handle_read(fake_read_args(3, 0, 0)).unwrap();
        let read5 = server.handle_read(fake_read_args(4, 0, 10)).unwrap();

        // Correctness
        assert_eq!(read1.attributes.size, 27);
        assert_eq!(read2.attributes.size, 27);
        assert_eq!(read3.attributes.size, 27);
        assert_eq!(read4.attributes.size, 27);
        assert_eq!(read5.attributes.size, 0);

        assert_eq!(read1.attributes.fid, 3);
        assert_eq!(read2.attributes.fid, 3);
        assert_eq!(read3.attributes.fid, 3);
        assert_eq!(read4.attributes.fid, 3);
        assert_eq!(read5.attributes.fid, 4);

        assert_eq!(read1.attributes.type_, ZipFtype::NFREG);
        assert_eq!(read2.attributes.type_, ZipFtype::NFREG);
        assert_eq!(read3.attributes.type_, ZipFtype::NFREG);
        assert_eq!(read4.attributes.type_, ZipFtype::NFREG);
        assert_eq!(read5.attributes.type_, ZipFtype::NFREG);

        assert_eq!(&read1.data[..], "bcdefghijk".as_bytes());
        assert_eq!(&read2.data[..], "abcdefghijklmnopqrstuvwxyz\n".as_bytes());
        assert_eq!(&read3.data[..], "".as_bytes());
        assert_eq!(&read4.data[..], "".as_bytes());
        assert_eq!(&read5.data[..], "".as_bytes());
    })
}

#[test]
fn test_nfs_getattr() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // LOOKUP a bunch of things
        let attr1 = server.handle_getattr(ZipFileHandle::new(1)).unwrap();
        let attr2 = server.handle_getattr(ZipFileHandle::new(2)).unwrap();
        let attr3 = server.handle_getattr(ZipFileHandle::new(3)).unwrap();
        let attr4 = server.handle_getattr(ZipFileHandle::new(4)).unwrap();
        let attr5 = server.handle_getattr(ZipFileHandle::new(5)).unwrap();
        let attr6 = server.handle_getattr(ZipFileHandle::new(6));
        let attr7 = server.handle_getattr(ZipFileHandle::new(7));
        let attr8 = server.handle_getattr(ZipFileHandle::new(8));

        // Correctness
        assert!(attr6.is_err());
        match attr6.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_STALE, _) => {}
            _ => assert!(false),
        }

        assert!(attr7.is_err());
        match attr7.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_STALE, _) => {}
            _ => assert!(false),
        }

        assert!(attr8.is_err());
        match attr8.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_STALE, _) => {}
            _ => assert!(false),
        }

        // For the two files
        assert_eq!(attr1.attributes.fid, 1);
        assert_eq!(attr2.attributes.fid, 2);
        assert_eq!(attr3.attributes.fid, 3);
        assert_eq!(attr4.attributes.fid, 4);
        assert_eq!(attr5.attributes.fid, 5);

        assert_eq!(attr1.attributes.type_, ZipFtype::NFDIR);
        assert_eq!(attr2.attributes.type_, ZipFtype::NFDIR);
        assert_eq!(attr3.attributes.type_, ZipFtype::NFREG);
        assert_eq!(attr4.attributes.type_, ZipFtype::NFREG);
        assert_eq!(attr5.attributes.type_, ZipFtype::NFDIR);
    })
}

#[test]
fn test_fs_create_obj() {
    run_with_clone_fs("test_files/test1/", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Check that objects do not exist
        assert!(!fspath.join("0/9").exists());
        assert!(!fspath.join("0/9.myfile.txt").exists());
        assert!(!fspath.join("0/10").exists());
        assert!(!fspath.join("0/10.mydir").exists());

        // Create a couple of objects
        let create1 = server
            .fs_create_obj(fspath.join("0"), "myfile.txt", true)
            .unwrap(); // file
        let create2 = server
            .fs_create_obj(fspath.join("0"), "mydir", false)
            .unwrap(); // dir
        // TODO: possibly add more tests

        // Correctness
        assert_eq!(create1, (9, fspath.join("0/9")));
        assert_eq!(create2, (10, fspath.join("0/10")));

        // Check that they exist
        assert!(fspath.join("0/9").exists());
        assert!(fspath.join("0/9").is_file());
        assert!(fspath.join("0/9.myfile.txt").exists());
        assert!(fspath.join("0/9.myfile.txt").is_file());
        assert!(fspath.join("0/10").exists());
        assert!(fspath.join("0/10").is_dir());
        assert!(fspath.join("0/10.mydir").exists());
        assert!(fspath.join("0/10.mydir").is_file());
    })
}

fn create_object(is_file: bool) {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Call create_object repeatedly
        let create1 = server
            .create_object(fake_create_args(0, "myobj"), is_file)
            .unwrap();
        let create2 = server.create_object(fake_create_args(0, "foo"), is_file);
        let create3 = server.create_object(fake_create_args(2, "zee.txt"), is_file);

        // Correctness
        assert_eq!(create1.file.fid, 9);

        assert!(create2.is_err());
        match create2.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_EXIST, _) => {}
            _ => assert!(false),
        }

        assert!(create3.is_err());
        match create3.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_EXIST, _) => {}
            _ => assert!(false),
        }
    })
}

fn create_object_concurrent(is_file: bool) {
    use std::fs::{create_dir, remove_dir_all, File};
    use std::io::Write;
    use std::sync::Arc;
    use std::thread;

    let _ctl = CONC_TEST_LOCK.lock();

    // Cleanup after previous attempts
    let fspath: PathBuf = format!("test_files/test_create_object_concurrent_{}", is_file).into();
    if fspath.exists() {
        remove_dir_all(&fspath).unwrap();
    }

    // Populate the new directory
    create_dir(&fspath).unwrap();
    File::create(fspath.join("0.root")).unwrap();
    create_dir(fspath.join("0")).unwrap();
    File::create(fspath.join("counter"))
        .unwrap()
        .write(&[1, 0, 0, 0, 0, 0, 0, 0])
        .unwrap();

    // If this is 1000, then initial remove_dir_all() or child.join() fail
    const NTHREADS: usize = 1000;

    // Create a new scope because server drop interfers with test cleanup
    {
        let server = Arc::new(ZippynfsServer::new(fspath.clone()));
        let mut children = Vec::with_capacity(NTHREADS);

        // Create a bunch of racing threads
        for _ in 0..NTHREADS {
            let server = server.clone();
            children.push(thread::spawn(move || {
                let _ = server.create_object(fake_create_args(0, "myobj"), is_file);
            }));
        }

        // Wait for all threads to exit
        for child in children {
            child.join().unwrap();
        }

        // No checks for correctness
    }

    // Only one numbered file and named file got created
    assert!(fspath.join("0/2.myobj").exists());
    assert!(fspath.join("0/2").exists());
    for i in 1..NTHREADS {
        if i != 2 {
            assert!(!fspath.join(format!("0/{}.myobj", i)).exists());
            assert!(!fspath.join(format!("0/{}", i)).exists());
        }
    }

    // Cleanup afterwards, if needed
    remove_dir_all(&fspath).unwrap();
}

#[test]
fn test_nfs_mkdir() {
    create_object(false)
}

#[test]
fn test_mkdir_concurrent() {
    create_object_concurrent(false)
}

#[test]
fn test_nfs_create() {
    create_object(true)
}

#[test]
fn test_create_concurrent() {
    create_object_concurrent(true)
}

#[test]
fn test_fs_delete_obj() {
    run_with_clone_fs("test_files/test1/", true, |fspath| {
        // Do some cleanup (to get around git hackery)
        cleanup_git_hackery_test1(fspath);

        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Delete a couple of items
        let del4 = server.fs_delete_obj(fspath.join("0"), 4, "baz.txt", true); // file
        let del5 = server.fs_delete_obj(fspath.join("0"), 5, "bazee", false); // dir

        // Correctness
        assert_eq!(del4.ok().unwrap(), ());
        assert_eq!(del5.ok().unwrap(), ());

        // Check that they no longer exist
        let lookup4 = server.handle_lookup(fake_dir_op_args(0, "baz.txt"));
        let lookup5 = server.handle_lookup(fake_dir_op_args(0, "bazee"));

        assert!(lookup4.is_err());
        match lookup4.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }

        assert!(lookup5.is_err());
        match lookup5.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }
    })
}

#[test]
fn test_nfs_rmdir() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Do some cleanup (to get around git hackery)
        cleanup_git_hackery_test1(fspath);

        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Call RMDIR
        let rmdir1 = server.handle_rmdir(fake_dir_op_args(0, "foo"));
        let rmdir3 = server.handle_rmdir(fake_dir_op_args(2, "zee.txt"));
        let _rmdir5 = server.handle_rmdir(fake_dir_op_args(0, "bazee")).unwrap();
        let rmdir8 = server.handle_rmdir(fake_dir_op_args(0, "baz"));

        // Correctness
        assert!(rmdir1.is_err());
        match rmdir1.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOTEMPTY, _) => {}
            _ => assert!(false),
        }

        assert!(rmdir3.is_err());
        match rmdir3.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOTDIR, _) => {}
            _ => assert!(false),
        }

        assert!(rmdir8.is_err());
        match rmdir8.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }

        // Make sure it is actually deleted
        let lookup5 = server.handle_lookup(fake_dir_op_args(0, "bazee"));
        assert!(lookup5.is_err());
        match lookup5.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }
    })
}

#[test]
fn test_nfs_remove() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Do some cleanup (to get around git hackery)
        cleanup_git_hackery_test1(fspath);

        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Call RMDIR
        let rm1 = server.handle_remove(fake_dir_op_args(0, "foo"));
        let _rm3 = server
            .handle_remove(fake_dir_op_args(2, "zee.txt"))
            .unwrap();
        let rm5 = server.handle_remove(fake_dir_op_args(0, "bazee"));
        let rm8 = server.handle_remove(fake_dir_op_args(0, "baz"));

        // Correctness
        assert!(rm1.is_err());
        match rm1.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_ISDIR, _) => {}
            _ => assert!(false),
        }

        assert!(rm5.is_err());
        match rm5.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_ISDIR, _) => {}
            _ => assert!(false),
        }

        assert!(rm8.is_err());
        match rm8.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }

        // Make sure it is actually deleted
        let lookup3 = server.handle_lookup(fake_dir_op_args(2, "zee.txt"));
        assert!(lookup3.is_err());
        match lookup3.map_err(|e| e.into()).err().unwrap() {
            ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _) => {}
            _ => assert!(false),
        }
    })
}

#[test]
fn test_nfs_readdir() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Do some cleanup (to get around git hackery)
        cleanup_git_hackery_test1(fspath);

        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Call RMDIR
        let readdir0 = server.handle_readdir(ZipReadDirArgs::new(ZipFileHandle::new(0)));

        // Correctness
        match readdir0 {
            Ok(ZipReadDirRes { entries }) => {
                let correct_entries: HashSet<(u64, String)> =
                    vec![(1, "foo"), (4, "baz.txt"), (5, "bazee")]
                        .into_iter()
                        .map(|(fid, fname)| (fid, fname.to_owned()))
                        .collect();

                let actual_entries = entries
                    .into_iter()
                    .map(|ZipDirEntry { fid, fname }| (fid as u64, fname))
                    .collect();

                // Same set
                assert_eq!(correct_entries, actual_entries);
            }
            _ => assert!(false),
        }
    })
}

#[test]
fn test_nfs_rename() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Move some stuff

        // 1. file that exists to new file
        let _move3 = server
            .handle_rename(fake_rename_args(2, "zee.txt", 1, "zee.mv.txt"))
            .unwrap();

        // 2. dir that exists to new dir
        let _move1 = server
            .handle_rename(fake_rename_args(0, "foo", 5, "foo.mv"))
            .unwrap();

        // 3. file that doesn't exist
        let move3_again = server.handle_rename(fake_rename_args(2, "zee.txt", 1, "zee.mv.txt"));

        // 4. dir that doesn't exist
        let move1_again = server.handle_rename(fake_rename_args(0, "foo", 5, "foo.mv"));

        // 5. file that does exists to dir that doesn't
        let move3_again2 =
            server.handle_rename(fake_rename_args(1, "zee.mv.txt", 6, "zee.mv.again2.txt"));

        // 6. dir that does exists to dir that doesn't
        let move1_again2 = server.handle_rename(fake_rename_args(5, "foo.mv", 6, "foo.mv.again2"));

        // 7. file that exists to file that already exists
        let move3_again3 = server.handle_rename(fake_rename_args(1, "zee.mv.txt", 0, "baz.txt"));

        // 8. file that exists to dir that already exists
        let move3_again4 = server.handle_rename(fake_rename_args(1, "zee.mv.txt", 0, "bazee"));

        // 9. dir that exists to dir that already exists
        let move1_again3 = server.handle_rename(fake_rename_args(5, "foo.mv", 0, "bazee"));

        // 10. dir that exists to file that already exists
        let move1_again4 = server.handle_rename(fake_rename_args(5, "foo.mv", 0, "baz.txt"));

        // 11. dir into itself
        let move1_again5 = server.handle_rename(fake_rename_args(5, "foo.mv", 1, "heheheh"));

        // Correctness

        // Make sure the old file was deleted and the new one created
        let find1_old = server.fs_find_by_name(fspath.join("0"), "foo").unwrap();
        let find1_new = server
            .fs_find_by_name(fspath.join("0/5"), "foo.mv")
            .unwrap();

        assert!(find1_old.is_none());
        assert_eq!(find1_new, Some(1));

        let find3_old = server
            .fs_find_by_name(fspath.join("0/5/1/2"), "zee.txt")
            .unwrap();
        let find3_new = server
            .fs_find_by_name(fspath.join("0/5/1"), "zee.mv.txt")
            .unwrap();

        assert!(find3_old.is_none());
        assert_eq!(find3_new, Some(3));

        // None of the other operations should have succeeded

        // 3
        match move3_again.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _)) => {}
            _ => assert!(false),
        }

        // 4
        match move1_again.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_NOENT, _)) => {}
            _ => assert!(false),
        }

        // 5
        match move3_again2.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_STALE, _)) => {}
            _ => assert!(false),
        }

        // 6
        match move1_again2.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_STALE, _)) => {}
            _ => assert!(false),
        }

        // 7
        match move1_again3.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_EXIST, _)) => {}
            _ => assert!(false),
        }

        // 8
        match move1_again4.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_EXIST, _)) => {}
            _ => assert!(false),
        }

        // 9
        match move3_again3.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_EXIST, _)) => {}
            _ => assert!(false),
        }

        // 10
        match move3_again4.map_err(|e| e.into()) {
            Err(ZipError::Nfs(ZipErrorType::NFSERR_EXIST, _)) => {}
            _ => assert!(false),
        }

        // 11
        match move1_again5 {
            Err(_) => {}
            _ => assert!(false),
        }

    })

}

#[test]
fn test_nfs_rename_concurrent() {
    use std::fs::{create_dir, remove_dir_all, File};
    use std::io::Write;
    use std::sync::Arc;
    use std::thread;

    let _ctl = CONC_TEST_LOCK.lock();

    // Cleanup after previous attempts
    let fspath: PathBuf = "test_files/test_rename_concurrent".into();
    if fspath.exists() {
        remove_dir_all(&fspath).unwrap();
    }

    // Populate the new directory
    create_dir(&fspath).unwrap();
    File::create(fspath.join("0.root")).unwrap();
    create_dir(fspath.join("0")).unwrap();
    File::create(fspath.join("counter"))
        .unwrap()
        .write(&[1, 0, 0, 0, 0, 0, 0, 0])
        .unwrap();

    // If this is 1000, then initial remove_dir_all() or child.join() fail
    const NTHREADS: usize = 900;

    // Create a new scope because server drop interfers with test cleanup
    {
        let server = Arc::new(ZippynfsServer::new(fspath.clone()));
        let mut children = Vec::with_capacity(NTHREADS);

        // Create a bunch of racing threads
        for i in 0..NTHREADS {
            let server = server.clone();
            children.push(thread::spawn(move || {
                let old_name = format!("myobj{}", i);
                let _ = server
                    .create_object(fake_create_args(0, &old_name), i % 2 == 0)
                    .unwrap();
                let _ = server.handle_rename(fake_rename_args(0, &old_name, 0, "foo"));
            }));
        }

        // Wait for all threads to exit
        for child in children {
            child.join().unwrap();
        }

        // Correctness

        // At most one file called "foo" got created
        let find_foo = server.fs_find_by_name(fspath.join("0"), "foo").unwrap();

        if let Some(fid) = find_foo {
            for i in 1..NTHREADS {
                if i != fid {
                    assert!(!fspath.join(format!("0/{}.foo", i)).exists());
                }
            }
        }
    }

    // Cleanup afterwards, if needed
    remove_dir_all(&fspath).unwrap();
}

#[test]
fn test_nfs_statfs() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Make sure we can create a server
        let server = ZippynfsServer::new(fspath);
        let _statfs = server.handle_statfs(ZipFileHandle::new(0)).unwrap();
    })
}

#[test]
fn test_nfs_write() {
    run_with_clone_fs("test_files/test1", true, |fspath| {
        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Write a file
        let data = "Hello, World!".as_bytes();
        let write3 = server
            .handle_write(ZipWriteArgs::new(
                ZipFileHandle::new(3),
                0, // offset
                data.len() as i64, // count
                data.clone().into(),
                ZipWriteStable::FILE_SYNC,
            ))
            .unwrap();

        // Correctness
        assert_eq!(write3.count as usize, 13);
        assert_eq!(write3.committed, ZipWriteStable::FILE_SYNC);
        assert_eq!(write3.verf, 8); // server epoch

        // Check that the write happened
        assert_eq!(data.len(), 13);
        let mut buf = [0u8; 13];

        let fpath_numbered = fspath.join("0/1/2/3");
        let mut file = File::open(fpath_numbered).unwrap();

        assert_eq!(file.metadata().unwrap().len(), data.len() as u64);

        file.read_exact(&mut buf).unwrap();
        assert_eq!(buf, data);
    })
}
