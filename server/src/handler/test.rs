//! Unit tests for ZippynfsServer

use std::process::Command;
#[allow(unused_imports)]
use std::error::Error as std_err;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use regex::Regex;

use thrift::Error;

use zippyrpc::*;

use super::ZippynfsServer;
use super::errors::*;

fn cleanup_git_hackery_test1<P>(fspath: P)
where
    P: AsRef<Path>,
{
    use std::fs::remove_file;

    remove_file((&fspath).as_ref().join("0/5/32.empty")).unwrap();
    remove_file((&fspath).as_ref().join("0/6/33.empty")).unwrap();
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

#[test]
fn test_new() {
    // Make sure we can create a server
    let _server = ZippynfsServer::new("test_files/test1/");
}

#[test]
fn test_get_numbered_and_named_files() {
    // Create a server
    let server = ZippynfsServer::new("test_files/test1/");

    let re = Regex::new(r"^\d+$").unwrap();
    let path: &Path = "test_files/test1/0".as_ref();
    let path = path.to_path_buf();
    let (numbered_files, named_files) = server.get_numbered_and_named_files(&re, &path).unwrap();
    assert_eq!(numbered_files.len(), 4);
    assert_eq!(named_files.len(), 4);

    println!("{:?}", numbered_files);

    assert!(numbered_files.contains(
        &Path::new("test_files/test1/0/1").to_path_buf(),
    ));
    assert!(numbered_files.contains(
        &Path::new("test_files/test1/0/4").to_path_buf(),
    ));
    assert!(numbered_files.contains(
        &Path::new("test_files/test1/0/5").to_path_buf(),
    ));
    assert!(numbered_files.contains(
        &Path::new("test_files/test1/0/6").to_path_buf(),
    ));

    assert!(named_files.contains(
        &Path::new("test_files/test1/0/1.foo").to_path_buf(),
    ));
    assert!(named_files.contains(
        &Path::new("test_files/test1/0/4.baz.txt").to_path_buf(),
    ));
    assert!(named_files.contains(
        &Path::new("test_files/test1/0/5.bazee").to_path_buf(),
    ));
    assert!(named_files.contains(
        &Path::new("test_files/test1/0/7.deleted.txt").to_path_buf(),
    ));
}

#[test]
fn test_fs_find_by_fid() {
    // Create a server
    let server = ZippynfsServer::new("test_files/test1");

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
    assert_eq!(path0, Ok(Some("test_files/test1/0".into())));
    assert_eq!(path1, Ok(Some("test_files/test1/0/1".into())));
    assert_eq!(path2, Ok(Some("test_files/test1/0/1/2".into())));
    assert_eq!(path3, Ok(Some("test_files/test1/0/1/2/3".into())));
    assert_eq!(path4, Ok(Some("test_files/test1/0/4".into())));
    assert_eq!(path5, Ok(Some("test_files/test1/0/5".into())));
    assert_eq!(path6, Ok(None));
    assert_eq!(path7, Ok(None));
}

#[test]
fn test_fs_find_by_name() {
    // Create a server
    let server = ZippynfsServer::new("test_files/test1");

    // Look for a bunch of stuff, and make sure we get the right results
    let find1 = server.fs_find_by_name("test_files/test1/0".into(), "foo");
    let find2 = server.fs_find_by_name("test_files/test1/0/1".into(), "bar");
    let find3 = server.fs_find_by_name("test_files/test1/0/1/2".into(), "zee.txt");
    let find4 = server.fs_find_by_name("test_files/test1/0".into(), "baz.txt");
    let find5 = server.fs_find_by_name("test_files/test1/0".into(), "bazee");
    let find7 = server.fs_find_by_name("test_files/test1/0".into(), "deleted.txt");
    let find8 = server.fs_find_by_name("test_files/test1/0".into(), ".");
    let find9 = server.fs_find_by_name("test_files/test1/0".into(), "fignewton");

    // Correctness
    assert_eq!(find1, Ok(Some(1)));
    assert_eq!(find2, Ok(Some(2)));
    assert_eq!(find3, Ok(Some(3)));
    assert_eq!(find4, Ok(Some(4)));
    assert_eq!(find5, Ok(Some(5)));
    assert_eq!(find7, Ok(None));
    assert_eq!(find8, Ok(None));
    assert_eq!(find9, Ok(None));
}

#[test]
fn test_fs_get_attr() {
    // Create a server
    let server = ZippynfsServer::new("test_files/test1");

    // Get attributes for a bunch of files
    let attr0 = server.fs_get_attr("test_files/test1/0".into(), 0);
    let attr1 = server.fs_get_attr("test_files/test1/0/1".into(), 1);
    let attr2 = server.fs_get_attr("test_files/test1/0/1/2".into(), 2);
    let attr3 = server.fs_get_attr("test_files/test1/0/1/2/3".into(), 3);
    let attr4 = server.fs_get_attr("test_files/test1/0/4".into(), 4);
    let attr5 = server.fs_get_attr("test_files/test1/0/5".into(), 5);

    // Correctness
    assert_eq!(attr0.fid, 0);
    assert_eq!(attr1.fid, 1);
    assert_eq!(attr2.fid, 2);
    assert_eq!(attr3.fid, 3);
    assert_eq!(attr4.fid, 4);
    assert_eq!(attr5.fid, 5);

    // For files:
    assert_eq!(attr3.size, 0);
    assert_eq!(attr4.size, 0);

    assert_eq!(attr0.type_, ZipFtype::NFDIR);
    assert_eq!(attr1.type_, ZipFtype::NFDIR);
    assert_eq!(attr2.type_, ZipFtype::NFDIR);
    assert_eq!(attr3.type_, ZipFtype::NFREG);
    assert_eq!(attr4.type_, ZipFtype::NFREG);
    assert_eq!(attr5.type_, ZipFtype::NFDIR);
}

#[test]
fn test_nfs_lookup() {
    fn fake_dir_op_args(did: i64, filename: &str) -> ZipDirOpArgs {
        ZipDirOpArgs::new(ZipFileHandle::new(did), filename.to_owned())
    }

    // Create a server
    let server = ZippynfsServer::new("test_files/test1");

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
    match lookup7.err().unwrap() {
        Error::Application(err) => assert_eq!(err.message, NFSERR_NOENT),
        _ => assert!(false),
    }

    assert!(lookup8.is_err());
    match lookup8.err().unwrap() {
        Error::Application(err) => assert_eq!(err.message, NFSERR_NOENT),
        _ => assert!(false),
    }

    assert_eq!(lookup1.file.fid, 1);
    assert_eq!(lookup2.file.fid, 2);
    assert_eq!(lookup3.file.fid, 3);
    assert_eq!(lookup4.file.fid, 4);
    assert_eq!(lookup5.file.fid, 5);

    // For the two files
    assert_eq!(lookup3.attributes.size, 0);
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
}

#[test]
fn test_nfs_getattr() {
    // Create a server
    let server = ZippynfsServer::new("test_files/test1");

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
    match attr6.err().unwrap() {
        Error::Application(err) => assert_eq!(err.message, NFSERR_STALE),
        _ => assert!(false),
    }

    assert!(attr7.is_err());
    match attr7.err().unwrap() {
        Error::Application(err) => assert_eq!(err.message, NFSERR_STALE),
        _ => assert!(false),
    }

    assert!(attr8.is_err());
    match attr8.err().unwrap() {
        Error::Application(err) => assert_eq!(err.message, NFSERR_STALE),
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
}

#[test]
fn test_fs_delete_obj() {
    fn fake_dir_op_args(did: i64, filename: &str) -> ZipDirOpArgs {
        ZipDirOpArgs::new(ZipFileHandle::new(did), filename.to_owned())
    }

    run_with_clone_fs("test_files/test1/", false, |fspath| {
        // Do some cleanup (to get around git hackery)
        cleanup_git_hackery_test1(fspath);

        // Create a server
        let server = ZippynfsServer::new(fspath);

        // Delete a couple of items
        let del4 = server.fs_delete_obj(fspath.join("0"), 4, "baz.txt", true); // file
        let del5 = server.fs_delete_obj(fspath.join("0"), 5, "bazee", false); // dir

        // Correctness
        assert_eq!(del4, Ok(()));
        assert_eq!(del5, Ok(()));

        // Check that they no longer exist
        let lookup4 = server.handle_lookup(fake_dir_op_args(0, "baz.txt"));
        let lookup5 = server.handle_lookup(fake_dir_op_args(0, "bazee"));

        assert!(lookup4.is_err());
        match lookup4.err().unwrap() {
            Error::Application(err) => assert_eq!(err.message, NFSERR_NOENT),
            _ => assert!(false),
        }

        assert!(lookup5.is_err());
        match lookup5.err().unwrap() {
            Error::Application(err) => assert_eq!(err.message, NFSERR_NOENT),
            _ => assert!(false),
        }
    })
}
