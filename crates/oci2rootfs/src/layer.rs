use std::io::Read;

use tar::{Archive, EntryType};

use crate::error::Result;
use crate::ext4::Ext4Writer;

/// Apply a single OCI layer (tar stream) to the ext4 writer.
///
/// Processes tar entries in order, handling whiteout files per the OCI spec.
pub fn apply_layer(reader: impl Read, writer: &mut Ext4Writer) -> Result<()> {
    let mut archive = Archive::new(reader);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let raw_path = entry.path()?.to_path_buf();
        let path_str = normalize_path(&raw_path);

        // Skip root directory entry
        if path_str == "/" {
            continue;
        }

        // Check for whiteout markers
        if let Some(whiteout) = parse_whiteout(&path_str) {
            match whiteout {
                Whiteout::Delete(target) => {
                    delete_entry(writer, &target)?;
                }
                Whiteout::Opaque(dir) => {
                    clear_directory(writer, &dir)?;
                }
            }
            continue;
        }

        // Process normal tar entries
        let entry_type = entry.header().entry_type();
        match entry_type {
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
                if let Some(target) = entry.link_name()? {
                    let target_str = target.to_string_lossy().to_string();
                    writer.symlink(&target_str, &path_str)?;
                }
            }
            EntryType::Link => {
                if let Some(target) = entry.link_name()? {
                    let target_str = normalize_path(&target);
                    writer.link(&target_str, &path_str)?;
                }
            }
            EntryType::Char | EntryType::Block => {
                eprintln!(
                    "warning: skipping device node: {} (ext4-lwext4 has no mknod support)",
                    path_str
                );
            }
            EntryType::Fifo => {
                eprintln!("warning: skipping FIFO: {}", path_str);
            }
            _ => {
                // XHeader, GNULongName, GNULongLink, etc.
                // The tar crate handles these internally as metadata for subsequent entries.
            }
        }
    }

    Ok(())
}

/// Whiteout type detected from a path.
enum Whiteout {
    /// Delete a specific file/directory.
    Delete(String),
    /// Clear all entries in a directory (opaque whiteout).
    Opaque(String),
}

/// Parse a path to detect OCI whiteout markers.
///
/// - `.wh..wh..opq` in a directory → Opaque(parent_dir)
/// - `.wh.<name>` in a directory → Delete(parent_dir/<name>)
fn parse_whiteout(path: &str) -> Option<Whiteout> {
    let file_name = path.rsplit('/').next()?;

    if file_name == ".wh..wh..opq" {
        // Opaque whiteout: clear the parent directory
        let parent = parent_dir(path);
        return Some(Whiteout::Opaque(parent));
    }

    if let Some(name) = file_name.strip_prefix(".wh.")
        && !name.is_empty()
    {
        // Regular whiteout: delete the named entry
        let parent = parent_dir(path);
        let target = if parent == "/" {
            format!("/{name}")
        } else {
            format!("{parent}/{name}")
        };
        return Some(Whiteout::Delete(target));
    }

    None
}

/// Get the parent directory of a path.
fn parent_dir(path: &str) -> String {
    let path = path.trim_end_matches('/');
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(pos) => path[..pos].to_string(),
    }
}

/// Normalize a tar entry path to an absolute ext4 path.
///
/// - Strips leading "./" prefix
/// - Ensures leading "/"
/// - Removes trailing "/" (except for root)
fn normalize_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let s = s.strip_prefix("./").unwrap_or(&s);
    let s = s.strip_prefix('.').unwrap_or(s);

    if s.is_empty() || s == "/" {
        "/".to_string()
    } else if s.starts_with('/') {
        s.trim_end_matches('/').to_string()
    } else {
        format!("/{}", s.trim_end_matches('/'))
    }
}

/// Delete a file or directory from the ext4 image.
fn delete_entry(writer: &mut Ext4Writer, path: &str) -> Result<()> {
    if writer.is_dir(path) {
        writer.rmdir(path)?;
    } else if writer.exists(path) {
        writer.remove(path)?;
    }
    Ok(())
}

/// Clear all entries in a directory (opaque whiteout).
fn clear_directory(writer: &mut Ext4Writer, dir: &str) -> Result<()> {
    if !writer.is_dir(dir) {
        return Ok(());
    }

    let entries = writer.list_dir(dir)?;
    for name in entries {
        let child_path = if dir == "/" {
            format!("/{name}")
        } else {
            format!("{dir}/{name}")
        };
        delete_entry(writer, &child_path)?;
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

    const TEST_SIZE: u64 = 16 * 1024 * 1024;

    fn create_writer() -> (Ext4Writer, NamedTempFile) {
        let file = NamedTempFile::new().unwrap();
        let writer = Ext4Writer::create(file.path(), TEST_SIZE).unwrap();
        (writer, file)
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
    fn test_normalize_path() {
        assert_eq!(
            normalize_path(std::path::Path::new("./usr/bin/foo")),
            "/usr/bin/foo"
        );
        assert_eq!(
            normalize_path(std::path::Path::new("usr/bin/foo")),
            "/usr/bin/foo"
        );
        assert_eq!(
            normalize_path(std::path::Path::new("/usr/bin/foo")),
            "/usr/bin/foo"
        );
        assert_eq!(normalize_path(std::path::Path::new(".")), "/");
        assert_eq!(normalize_path(std::path::Path::new("./")), "/");
        assert_eq!(normalize_path(std::path::Path::new("./etc/")), "/etc");
    }

    #[test]
    fn test_parse_whiteout_delete() {
        let wh = parse_whiteout("/etc/.wh.resolv.conf");
        assert!(matches!(wh, Some(Whiteout::Delete(ref p)) if p == "/etc/resolv.conf"));
    }

    #[test]
    fn test_parse_whiteout_opaque() {
        let wh = parse_whiteout("/var/cache/.wh..wh..opq");
        assert!(matches!(wh, Some(Whiteout::Opaque(ref p)) if p == "/var/cache"));
    }

    #[test]
    fn test_parse_whiteout_none() {
        assert!(parse_whiteout("/usr/bin/foo").is_none());
        assert!(parse_whiteout("/etc/passwd").is_none());
    }

    #[test]
    fn test_parse_whiteout_root_level() {
        let wh = parse_whiteout("/.wh.foo");
        assert!(matches!(wh, Some(Whiteout::Delete(ref p)) if p == "/foo"));
    }

    #[test]
    fn test_parent_dir_root_child() {
        assert_eq!(parent_dir("/foo"), "/");
    }

    #[test]
    fn test_parent_dir_nested() {
        assert_eq!(parent_dir("/a/b/c"), "/a/b");
    }

    #[test]
    fn test_parent_dir_root() {
        assert_eq!(parent_dir("/"), "/");
    }

    #[test]
    fn test_normalize_path_trailing_slash() {
        assert_eq!(normalize_path(std::path::Path::new("/usr/")), "/usr");
    }

    #[test]
    #[serial]
    fn apply_layer_regular_files() {
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
    fn apply_layer_directories() {
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
    fn apply_layer_symlink() {
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
    fn apply_layer_hardlink() {
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
    fn apply_layer_whiteout_delete() {
        let (mut writer, _file) = create_writer();
        let base = build_tar(&[TarEntry::File {
            path: "etc/removeme.conf",
            data: b"config".to_vec(),
            mode: 0o644,
        }]);
        apply_layer(Cursor::new(base), &mut writer).unwrap();
        assert!(writer.exists("/etc/removeme.conf"));

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
    fn apply_layer_whiteout_opaque() {
        let (mut writer, _file) = create_writer();
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

        let overlay = build_tar(&[TarEntry::File {
            path: "var/cache/.wh..wh..opq",
            data: vec![],
            mode: 0o644,
        }]);
        apply_layer(Cursor::new(overlay), &mut writer).unwrap();
        assert!(!writer.exists("/var/cache/a.txt"));
        assert!(!writer.exists("/var/cache/b.txt"));
        assert!(writer.is_dir("/var/cache"));

        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn apply_layer_gzip_after_decompression() {
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
