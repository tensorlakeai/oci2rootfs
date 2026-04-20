use std::io::Read;

use tar::{Archive, EntryType};

use crate::error::{Error, Result};
use crate::ext4::Ext4Writer;
use crate::path::{Whiteout, join, parent_of, parse_oci_whiteout, sanitize_entry_path};

/// Apply a single OCI layer (tar stream) to the ext4 writer.
///
/// Processes tar entries in order, sanitizing each entry path per
/// [`sanitize_entry_path`] (rejecting `..` components and NUL bytes) and
/// applying OCI whiteout semantics (`.wh.<name>` deletes, `.wh..wh..opq`
/// clears the parent directory).
pub fn apply_layer(reader: impl Read, writer: &mut Ext4Writer) -> Result<()> {
    let mut archive = Archive::new(reader);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let raw_path = entry.path()?.into_owned();
        let path_str = sanitize_entry_path(&raw_path)?;

        if path_str == "/" {
            continue;
        }

        if let Some(leaf) = raw_path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(parse_oci_whiteout)
        {
            let parent = parent_of(&path_str);
            match leaf {
                Whiteout::Delete(name) => writer.delete(&join(&parent, name))?,
                Whiteout::Opaque => writer.clear_dir(&parent)?,
            }
            continue;
        }

        match entry.header().entry_type() {
            EntryType::Regular | EntryType::GNUSparse => {
                let mode = entry.header().mode().unwrap_or(0o644);
                let uid = entry.header().uid().unwrap_or(0) as u32;
                let gid = entry.header().gid().unwrap_or(0) as u32;
                writer.write_file(&path_str, &mut entry, mode, uid, gid)?;
            }
            EntryType::Directory => {
                let mode = entry.header().mode().unwrap_or(0o755);
                let uid = entry.header().uid().unwrap_or(0) as u32;
                let gid = entry.header().gid().unwrap_or(0) as u32;
                writer.mkdir_p(&path_str, mode)?;
                writer.set_owner(&path_str, uid, gid)?;
            }
            EntryType::Symlink => {
                let target = entry.link_name()?.ok_or_else(|| {
                    Error::InvalidTarPath(format!("symlink without target: {path_str}"))
                })?;
                let target_str = target
                    .to_str()
                    .ok_or_else(|| {
                        Error::InvalidTarPath(format!("non-UTF-8 symlink target: {path_str}"))
                    })?
                    .to_string();
                reject_nul(&target_str, &path_str)?;
                writer.symlink(&target_str, &path_str)?;
            }
            EntryType::Link => {
                let target = entry.link_name()?.ok_or_else(|| {
                    Error::InvalidTarPath(format!("hardlink without target: {path_str}"))
                })?;
                let target_str = sanitize_entry_path(&target)?;
                writer.link(&target_str, &path_str)?;
            }
            EntryType::Char | EntryType::Block | EntryType::Fifo => {
                // arcbox-ext4 has no mknod support; skip.
            }
            _ => {
                // XHeader, GNULongName, GNULongLink, etc.
                // The tar crate handles these internally as metadata for subsequent entries.
            }
        }
    }

    Ok(())
}

fn reject_nul(value: &str, owner_path: &str) -> Result<()> {
    if value.contains('\0') {
        return Err(Error::InvalidTarPath(format!(
            "NUL byte in tar entry target at {owner_path}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use serial_test::serial;
    use std::io::Cursor;
    use tempfile::NamedTempFile;

    use crate::ext4::Ext4Writer;
    use crate::ext4_options::Ext4Options;

    const TEST_SIZE: u64 = 16 * 1024 * 1024;

    fn create_writer() -> (Ext4Writer, NamedTempFile) {
        let file = NamedTempFile::new().unwrap();
        let writer = Ext4Writer::create(file.path(), TEST_SIZE, &Ext4Options::default()).unwrap();
        (writer, file)
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
                    builder.append_link(&mut header, path, target).unwrap();
                }
                TarEntry::Hardlink { path, target } => {
                    let mut header = tar::Header::new_gnu();
                    header.set_size(0);
                    header.set_mode(0o644);
                    header.set_uid(0);
                    header.set_gid(0);
                    header.set_entry_type(tar::EntryType::Link);
                    header.set_cksum();
                    builder.append_link(&mut header, path, target).unwrap();
                }
            }
        }
        builder.into_inner().unwrap()
    }

    #[test]
    #[serial]
    fn applies_regular_files_and_directories() {
        let (mut writer, _file) = create_writer();
        let tar_data = build_tar(&[
            TarEntry::Dir {
                path: "etc/",
                mode: 0o755,
            },
            TarEntry::File {
                path: "etc/hello.txt",
                data: b"hello".to_vec(),
                mode: 0o644,
            },
        ]);
        apply_layer(Cursor::new(tar_data), &mut writer).unwrap();
        assert!(writer.is_dir("/etc"));
        assert!(writer.exists("/etc/hello.txt"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn applies_symlink_and_hardlink() {
        let (mut writer, _file) = create_writer();
        let tar_data = build_tar(&[
            TarEntry::File {
                path: "target.txt",
                data: b"data".to_vec(),
                mode: 0o644,
            },
            TarEntry::Symlink {
                path: "slink",
                target: "/target.txt",
            },
            TarEntry::Hardlink {
                path: "hlink",
                target: "target.txt",
            },
        ]);
        apply_layer(Cursor::new(tar_data), &mut writer).unwrap();
        assert!(writer.exists("/slink"));
        assert!(writer.exists("/hlink"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn whiteout_delete_removes_prior_entry() {
        let (mut writer, _file) = create_writer();
        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "etc/removeme.conf",
                data: b"config".to_vec(),
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();
        assert!(writer.exists("/etc/removeme.conf"));

        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "etc/.wh.removeme.conf",
                data: vec![],
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();
        assert!(!writer.exists("/etc/removeme.conf"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn whiteout_opaque_clears_directory_contents() {
        let (mut writer, _file) = create_writer();
        apply_layer(
            Cursor::new(build_tar(&[
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
            ])),
            &mut writer,
        )
        .unwrap();

        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "var/cache/.wh..wh..opq",
                data: vec![],
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();

        assert!(writer.is_dir("/var/cache"));
        assert!(!writer.exists("/var/cache/a.txt"));
        assert!(!writer.exists("/var/cache/b.txt"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn whiteout_opaque_recursively_clears_nested_directories() {
        let (mut writer, _file) = create_writer();
        apply_layer(
            Cursor::new(build_tar(&[
                TarEntry::Dir {
                    path: "cache/",
                    mode: 0o755,
                },
                TarEntry::Dir {
                    path: "cache/sub/",
                    mode: 0o755,
                },
                TarEntry::File {
                    path: "cache/sub/nested.txt",
                    data: b"x".to_vec(),
                    mode: 0o644,
                },
            ])),
            &mut writer,
        )
        .unwrap();

        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "cache/.wh..wh..opq",
                data: vec![],
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();

        assert!(writer.is_dir("/cache"));
        assert!(!writer.exists("/cache/sub"));
        assert!(!writer.exists("/cache/sub/nested.txt"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn multi_layer_delete_then_recreate() {
        let (mut writer, _file) = create_writer();
        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "file.txt",
                data: b"first".to_vec(),
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();
        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: ".wh.file.txt",
                data: vec![],
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();
        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "file.txt",
                data: b"third".to_vec(),
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();

        assert!(writer.exists("/file.txt"));
        writer.finish().unwrap();
    }

    /// Build a single-entry tar byte stream, bypassing the `tar::Builder`
    /// validation that refuses `..` in names/link targets. Needed so we can
    /// exercise the defensive path-sanitizer with input a normal builder
    /// could never produce.
    fn build_raw_tar(name: &str, typeflag: u8, link_name: Option<&str>, data: &[u8]) -> Vec<u8> {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_size(data.len() as u64);
        header.set_mtime(0);

        let bytes = header.as_mut_bytes();
        bytes[0..100].fill(0);
        let name_bytes = name.as_bytes();
        let n = name_bytes.len().min(100);
        bytes[..n].copy_from_slice(&name_bytes[..n]);

        bytes[156] = typeflag;

        if let Some(target) = link_name {
            bytes[157..257].fill(0);
            let target_bytes = target.as_bytes();
            let n = target_bytes.len().min(100);
            bytes[157..157 + n].copy_from_slice(&target_bytes[..n]);
        }

        header.set_cksum();

        let mut out = Vec::new();
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(data);
        let pad = (512 - (data.len() % 512)) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
        out.extend(std::iter::repeat_n(0u8, 1024));
        out
    }

    #[test]
    #[serial]
    fn rejects_parent_dir_traversal_in_entry_path() {
        let (mut writer, _file) = create_writer();
        let tar_data = build_raw_tar("../etc/passwd", b'0', None, b"hostile");
        let err = apply_layer(Cursor::new(tar_data), &mut writer).unwrap_err();
        assert!(
            matches!(err, Error::InvalidTarPath(_)),
            "expected InvalidTarPath, got {err:?}"
        );
    }

    #[test]
    #[serial]
    fn rejects_parent_dir_in_hardlink_target() {
        let (mut writer, _file) = create_writer();
        apply_layer(
            Cursor::new(build_tar(&[TarEntry::File {
                path: "target.txt",
                data: b"data".to_vec(),
                mode: 0o644,
            }])),
            &mut writer,
        )
        .unwrap();

        let tar_data = build_raw_tar("link", b'1', Some("../target.txt"), b"");
        let err = apply_layer(Cursor::new(tar_data), &mut writer).unwrap_err();
        assert!(matches!(err, Error::InvalidTarPath(_)));
    }

    #[test]
    #[serial]
    fn empty_tar_archive_is_ok() {
        let (mut writer, _file) = create_writer();
        apply_layer(Cursor::new(build_tar(&[])), &mut writer).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn applies_gzipped_layer() {
        let (mut writer, _file) = create_writer();
        let tar_data = build_tar(&[TarEntry::File {
            path: "compressed.txt",
            data: b"gzip content".to_vec(),
            mode: 0o644,
        }]);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        std::io::copy(&mut Cursor::new(&tar_data), &mut encoder).unwrap();
        let gzipped = encoder.finish().unwrap();

        let mut decoder = flate2::read::GzDecoder::new(Cursor::new(gzipped));
        apply_layer(&mut decoder, &mut writer).unwrap();
        assert!(writer.exists("/compressed.txt"));
        writer.finish().unwrap();
    }
}
