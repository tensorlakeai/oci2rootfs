use std::io::Read;

use tar::{Archive, EntryType};

use crate::error::Result;
use crate::ext4::Ext4Writer;

/// Apply a single OCI layer (tar stream) to the ext4 writer.
///
/// Processes tar entries in order, handling whiteout files per the OCI spec.
pub fn apply_layer(reader: impl Read, writer: &Ext4Writer) -> Result<()> {
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

    if let Some(name) = file_name.strip_prefix(".wh.") {
        if !name.is_empty() {
            // Regular whiteout: delete the named entry
            let parent = parent_dir(path);
            let target = if parent == "/" {
                format!("/{name}")
            } else {
                format!("{parent}/{name}")
            };
            return Some(Whiteout::Delete(target));
        }
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

    let result = if s.is_empty() || s == "/" {
        "/".to_string()
    } else if s.starts_with('/') {
        s.trim_end_matches('/').to_string()
    } else {
        format!("/{}", s.trim_end_matches('/'))
    };

    result
}

/// Delete a file or directory from the ext4 image.
fn delete_entry(writer: &Ext4Writer, path: &str) -> Result<()> {
    if writer.is_dir(path) {
        writer.rmdir(path)?;
    } else if writer.exists(path) {
        writer.remove(path)?;
    }
    Ok(())
}

/// Clear all entries in a directory (opaque whiteout).
fn clear_directory(writer: &Ext4Writer, dir: &str) -> Result<()> {
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

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path(std::path::Path::new("./usr/bin/foo")), "/usr/bin/foo");
        assert_eq!(normalize_path(std::path::Path::new("usr/bin/foo")), "/usr/bin/foo");
        assert_eq!(normalize_path(std::path::Path::new("/usr/bin/foo")), "/usr/bin/foo");
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
}
