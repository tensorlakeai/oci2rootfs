use std::io::Cursor;

use flate2::write::GzEncoder;
use flate2::Compression;
use oci2rootfs::ext4::Ext4Writer;
use oci2rootfs::layer::apply_layer;
use serial_test::serial;
use tempfile::NamedTempFile;

const TEST_SIZE: u64 = 16 * 1024 * 1024;

fn create_writer() -> (Ext4Writer, NamedTempFile) {
    let file = NamedTempFile::new().unwrap();
    let writer = Ext4Writer::create(file.path(), TEST_SIZE).unwrap();
    (writer, file)
}

/// Build a tar archive in memory with the given entries.
fn build_tar(entries: &[TarEntry]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for entry in entries {
        match entry {
            TarEntry::File { path, data, mode } => {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(*mode);
                header.set_uid(0);
                header.set_gid(0);
                header.set_entry_type(tar::EntryType::Regular);
                header.set_cksum();
                builder.append_data(&mut header, path, &data[..]).unwrap();
            }
            TarEntry::Dir { path, mode } => {
                let mut header = tar::Header::new_gnu();
                header.set_size(0);
                header.set_mode(*mode);
                header.set_uid(0);
                header.set_gid(0);
                header.set_entry_type(tar::EntryType::Directory);
                header.set_cksum();
                builder
                    .append_data(&mut header, path, &[] as &[u8])
                    .unwrap();
            }
            TarEntry::Symlink { path, target } => {
                let mut header = tar::Header::new_gnu();
                header.set_size(0);
                header.set_mode(0o777);
                header.set_uid(0);
                header.set_gid(0);
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_cksum();
                builder
                    .append_link(&mut header, path, target)
                    .unwrap();
            }
            TarEntry::Hardlink { path, target } => {
                let mut header = tar::Header::new_gnu();
                header.set_size(0);
                header.set_mode(0o644);
                header.set_uid(0);
                header.set_gid(0);
                header.set_entry_type(tar::EntryType::Link);
                header.set_cksum();
                builder
                    .append_link(&mut header, path, target)
                    .unwrap();
            }
        }
    }
    builder.into_inner().unwrap()
}

enum TarEntry {
    File {
        path: &'static str,
        data: Vec<u8>,
        mode: u32,
    },
    Dir {
        path: &'static str,
        mode: u32,
    },
    Symlink {
        path: &'static str,
        target: &'static str,
    },
    Hardlink {
        path: &'static str,
        target: &'static str,
    },
}

#[test]
#[serial]
fn test_apply_layer_regular_files() {
    let (mut writer, _file) = create_writer();
    let tar_data = build_tar(&[
        TarEntry::File {
            path: "hello.txt",
            data: b"hello".to_vec(),
            mode: 0o644,
        },
        TarEntry::File {
            path: "world.txt",
            data: b"world".to_vec(),
            mode: 0o644,
        },
    ]);
    apply_layer(Cursor::new(tar_data), &mut writer).unwrap();
    assert!(writer.exists("/hello.txt"));
    assert!(writer.exists("/world.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_apply_layer_directories() {
    let (mut writer, _file) = create_writer();
    let tar_data = build_tar(&[
        TarEntry::Dir {
            path: "etc/",
            mode: 0o755,
        },
        TarEntry::Dir {
            path: "etc/config/",
            mode: 0o755,
        },
    ]);
    apply_layer(Cursor::new(tar_data), &mut writer).unwrap();
    assert!(writer.is_dir("/etc"));
    assert!(writer.is_dir("/etc/config"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_apply_layer_symlink() {
    let (mut writer, _file) = create_writer();
    let tar_data = build_tar(&[
        TarEntry::File {
            path: "target.txt",
            data: b"data".to_vec(),
            mode: 0o644,
        },
        TarEntry::Symlink {
            path: "link.txt",
            target: "/target.txt",
        },
    ]);
    apply_layer(Cursor::new(tar_data), &mut writer).unwrap();
    assert!(writer.exists("/target.txt"));
    assert!(writer.exists("/link.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_apply_layer_hardlink() {
    let (mut writer, _file) = create_writer();
    let tar_data = build_tar(&[
        TarEntry::File {
            path: "original.txt",
            data: b"data".to_vec(),
            mode: 0o644,
        },
        TarEntry::Hardlink {
            path: "hardlink.txt",
            target: "original.txt",
        },
    ]);
    apply_layer(Cursor::new(tar_data), &mut writer).unwrap();
    assert!(writer.exists("/original.txt"));
    assert!(writer.exists("/hardlink.txt"));
    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_apply_layer_whiteout_delete() {
    let (mut writer, _file) = create_writer();

    // Base layer: create a file
    let base = build_tar(&[TarEntry::File {
        path: "etc/removeme.conf",
        data: b"config".to_vec(),
        mode: 0o644,
    }]);
    apply_layer(Cursor::new(base), &mut writer).unwrap();
    assert!(writer.exists("/etc/removeme.conf"));

    // Overlay layer: whiteout the file
    let overlay = build_tar(&[TarEntry::File {
        path: "etc/.wh.removeme.conf",
        data: vec![],
        mode: 0o644,
    }]);
    apply_layer(Cursor::new(overlay), &mut writer).unwrap();
    assert!(!writer.exists("/etc/removeme.conf"));

    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_apply_layer_whiteout_opaque() {
    let (mut writer, _file) = create_writer();

    // Base layer: create files in a directory
    let base = build_tar(&[
        TarEntry::Dir {
            path: "var/cache/",
            mode: 0o755,
        },
        TarEntry::File {
            path: "var/cache/a.txt",
            data: b"a".to_vec(),
            mode: 0o644,
        },
        TarEntry::File {
            path: "var/cache/b.txt",
            data: b"b".to_vec(),
            mode: 0o644,
        },
    ]);
    apply_layer(Cursor::new(base), &mut writer).unwrap();
    assert!(writer.exists("/var/cache/a.txt"));
    assert!(writer.exists("/var/cache/b.txt"));

    // Overlay layer: opaque whiteout clears directory contents
    let overlay = build_tar(&[TarEntry::File {
        path: "var/cache/.wh..wh..opq",
        data: vec![],
        mode: 0o644,
    }]);
    apply_layer(Cursor::new(overlay), &mut writer).unwrap();
    assert!(!writer.exists("/var/cache/a.txt"));
    assert!(!writer.exists("/var/cache/b.txt"));
    assert!(writer.is_dir("/var/cache")); // directory itself still exists

    writer.finish().unwrap();
}

#[test]
#[serial]
fn test_apply_layer_gzip() {
    let (mut writer, _file) = create_writer();

    let tar_data = build_tar(&[TarEntry::File {
        path: "compressed.txt",
        data: b"gzip content".to_vec(),
        mode: 0o644,
    }]);

    // Gzip-compress the tar
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::copy(&mut Cursor::new(&tar_data), &mut encoder).unwrap();
    let gzipped = encoder.finish().unwrap();

    // apply_layer expects a tar stream, so we need to decompress first
    // (in the real flow, oci.rs/pull.rs handles decompression)
    let decoder = flate2::read::GzDecoder::new(Cursor::new(gzipped));
    apply_layer(decoder, &mut writer).unwrap();
    assert!(writer.exists("/compressed.txt"));

    writer.finish().unwrap();
}
