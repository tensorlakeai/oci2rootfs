use std::io::Cursor;

use oci2rootfs::ext4::Ext4Writer;
use serial_test::serial;
use tempfile::NamedTempFile;

const TEST_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB

fn create_writer() -> (Ext4Writer, NamedTempFile) {
    let file = NamedTempFile::new().unwrap();
    let writer = Ext4Writer::create(file.path(), TEST_SIZE).unwrap();
    (writer, file)
}

#[test]
#[serial]
fn test_create_and_finish() {
    let (writer, file) = create_writer();
    writer.finish().unwrap();
    let metadata = std::fs::metadata(file.path()).unwrap();
    assert_eq!(metadata.len(), TEST_SIZE);
}

#[test]
#[serial]
fn test_mkdir_and_exists() {
    let (writer, _file) = create_writer();
    writer.mkdir_p("/testdir", 0o755).unwrap();
    assert!(writer.exists("/testdir"));
    assert!(writer.is_dir("/testdir"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_mkdir_idempotent() {
    let (writer, _file) = create_writer();
    writer.mkdir_p("/testdir", 0o755).unwrap();
    writer.mkdir_p("/testdir", 0o700).unwrap(); // should not fail
    assert!(writer.is_dir("/testdir"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_write_file() {
    let (writer, _file) = create_writer();
    let data = b"hello world";
    writer
        .write_file("/hello.txt", &mut Cursor::new(data), 0o644, 0, 0)
        .unwrap();
    assert!(writer.exists("/hello.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_write_file_overwrite() {
    let (writer, _file) = create_writer();
    writer
        .write_file("/file.txt", &mut Cursor::new(b"first"), 0o644, 0, 0)
        .unwrap();
    writer
        .write_file("/file.txt", &mut Cursor::new(b"second"), 0o644, 0, 0)
        .unwrap();
    assert!(writer.exists("/file.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_symlink() {
    let (writer, _file) = create_writer();
    writer
        .write_file("/target.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
        .unwrap();
    writer.symlink("/target.txt", "/link.txt").unwrap();
    assert!(writer.exists("/link.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_link() {
    let (writer, _file) = create_writer();
    writer
        .write_file("/original.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
        .unwrap();
    writer.link("/original.txt", "/hardlink.txt").unwrap();
    assert!(writer.exists("/hardlink.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_remove() {
    let (writer, _file) = create_writer();
    writer
        .write_file("/removeme.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
        .unwrap();
    assert!(writer.exists("/removeme.txt"));
    writer.remove("/removeme.txt").unwrap();
    assert!(!writer.exists("/removeme.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_rmdir() {
    let (writer, _file) = create_writer();
    writer.mkdir_p("/emptydir", 0o755).unwrap();
    assert!(writer.is_dir("/emptydir"));
    writer.rmdir("/emptydir").unwrap();
    assert!(!writer.is_dir("/emptydir"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_list_dir() {
    let (writer, _file) = create_writer();
    writer.mkdir_p("/parent", 0o755).unwrap();
    writer
        .write_file("/parent/a.txt", &mut Cursor::new(b"a"), 0o644, 0, 0)
        .unwrap();
    writer
        .write_file("/parent/b.txt", &mut Cursor::new(b"b"), 0o644, 0, 0)
        .unwrap();
    writer.mkdir_p("/parent/subdir", 0o755).unwrap();

    let mut entries = writer.list_dir("/parent").unwrap();
    entries.sort();
    assert_eq!(entries, vec!["a.txt", "b.txt", "subdir"]);
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_set_permissions_and_owner() {
    let (writer, _file) = create_writer();
    writer
        .write_file("/perm.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
        .unwrap();
    writer.set_permissions("/perm.txt", 0o600).unwrap();
    writer.set_owner("/perm.txt", 1000, 1000).unwrap();
    writer.finish().unwrap();
}
