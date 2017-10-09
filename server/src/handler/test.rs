//! Unit tests for ZippynfsServer

use std::fs::read_dir;
use std::path::Path;

use regex::Regex;

use super::ZippynfsServer;

#[test]
fn test_new() {
    // Make sure we can create a server
    let server = ZippynfsServer::new("test_files/test1/");
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