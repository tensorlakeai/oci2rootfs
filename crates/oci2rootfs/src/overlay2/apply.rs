//! Apply an overlay2 layer (diff directory) to an ext4 filesystem.
//!
//! Walks the directory tree and writes files, directories, and symlinks
//! directly into ext4. Handles both OCI-style whiteouts (`.wh.*`) and
//! overlay2-native whiteouts (character device 0/0). Preserves hardlinks
//! by tracking `(dev, ino)` pairs across the walk.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::error::Result;
use crate::ext4::Ext4Writer;

/// Inode key for hardlink detection: `(device, inode)`.
type InodeKey = (u64, u64);

/// Apply a single overlay2 layer to the ext4 writer.
pub(super) fn apply_directory_layer(diff_dir: &Path, writer: &mut Ext4Writer) -> Result<()> {
    let mut hardlinks: HashMap<InodeKey, String> = HashMap::new();
    walk(diff_dir, diff_dir, writer, &mut hardlinks)
}

/// Recursively walk `current` and write entries to ext4.
///
/// `root` is the layer's diff directory; paths are made absolute relative
/// to it (e.g. `root/usr/bin/foo` → `/usr/bin/foo` in ext4).
///
/// `hardlinks` tracks `(dev, ino) → ext4_path` so that files sharing an
/// inode on the host are written as hardlinks in ext4 instead of duplicates.
fn walk(
    root: &Path,
    current: &Path,
    writer: &mut Ext4Writer,
    hardlinks: &mut HashMap<InodeKey, String>,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)?.collect::<std::result::Result<Vec<_>, _>>()?;
    // Sort for deterministic output.
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let full_path = entry.path();
        let meta = fs::symlink_metadata(&full_path)?;

        let ext4_path = to_ext4_path(root, &full_path);

        // --- whiteout handling ---

        // OCI opaque whiteout: clear directory contents.
        if name_str == ".wh..wh..opq" {
            let parent = parent_of(&ext4_path);
            clear_directory(writer, &parent)?;
            continue;
        }

        // OCI-style deletion whiteout (.wh.<name>).
        if let Some(target_name) = name_str.strip_prefix(".wh.") {
            let parent = parent_of(&ext4_path);
            let target = if parent == "/" {
                format!("/{target_name}")
            } else {
                format!("{parent}/{target_name}")
            };
            delete_entry(writer, &target)?;
            continue;
        }

        // Overlay2-native whiteout: character device with rdev == 0.
        if is_chardev_whiteout(&meta) {
            delete_entry(writer, &ext4_path)?;
            continue;
        }

        // --- regular entries ---

        if meta.is_symlink() {
            let target = fs::read_link(&full_path)?;
            writer.symlink(&target.to_string_lossy(), &ext4_path)?;
        } else if meta.is_dir() {
            let (mode, uid, gid) = ownership(&meta);
            writer.mkdir_p(&ext4_path, mode)?;
            writer.set_owner(&ext4_path, uid, gid)?;
            walk(root, &full_path, writer, hardlinks)?;
        } else if meta.is_file() {
            // Hardlink detection: if we've already written a file with the
            // same (dev, ino), create a hardlink instead of a duplicate copy.
            if let Some(existing) = check_hardlink(&meta, hardlinks, &ext4_path) {
                writer.link(&existing, &ext4_path)?;
            } else {
                let (mode, uid, gid) = ownership(&meta);
                let mut file = fs::File::open(&full_path)?;
                writer.write_file(&ext4_path, &mut file, mode, uid, gid)?;
            }
        }
        // Skip sockets, FIFOs, and non-whiteout device nodes.
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// If `meta` has nlink > 1 and we've already seen its inode, return the
/// previously written ext4 path (so the caller can create a hardlink).
/// Otherwise record this inode and return `None`.
#[cfg(unix)]
fn check_hardlink(
    meta: &fs::Metadata,
    seen: &mut HashMap<InodeKey, String>,
    ext4_path: &str,
) -> Option<String> {
    if meta.nlink() <= 1 {
        return None;
    }
    let key = (meta.dev(), meta.ino());
    if let Some(existing) = seen.get(&key) {
        Some(existing.clone())
    } else {
        seen.insert(key, ext4_path.to_string());
        None
    }
}

#[cfg(not(unix))]
fn check_hardlink(
    _meta: &fs::Metadata,
    _seen: &mut HashMap<InodeKey, String>,
    _ext4_path: &str,
) -> Option<String> {
    None
}

/// Build the absolute ext4 path from a host filesystem path.
fn to_ext4_path(root: &Path, full_path: &Path) -> String {
    let rel = full_path.strip_prefix(root).unwrap_or(full_path);
    format!("/{}", rel.display())
}

/// Extract ownership and mode from metadata.
#[cfg(unix)]
fn ownership(meta: &fs::Metadata) -> (u32, u32, u32) {
    (meta.mode(), meta.uid(), meta.gid())
}

#[cfg(not(unix))]
fn ownership(_meta: &fs::Metadata) -> (u32, u32, u32) {
    (0o755, 0, 0)
}

/// Check whether metadata represents an overlay2-native whiteout (chardev 0/0).
#[cfg(unix)]
fn is_chardev_whiteout(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;
    meta.file_type().is_char_device() && meta.rdev() == 0
}

#[cfg(not(unix))]
fn is_chardev_whiteout(_meta: &fs::Metadata) -> bool {
    false
}

/// Parent ext4 path (`/a/b/c` → `/a/b`).
fn parent_of(path: &str) -> String {
    let path = path.trim_end_matches('/');
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(pos) => path[..pos].to_string(),
    }
}

/// Delete a file or directory from ext4.
fn delete_entry(writer: &mut Ext4Writer, path: &str) -> Result<()> {
    if writer.is_dir(path) {
        writer.rmdir(path)?;
    } else if writer.exists(path) {
        writer.remove(path)?;
    }
    Ok(())
}

/// Remove all children of a directory (opaque whiteout).
fn clear_directory(writer: &mut Ext4Writer, dir: &str) -> Result<()> {
    if !writer.is_dir(dir) {
        return Ok(());
    }
    let children = writer.list_dir(dir)?;
    for name in children {
        let child = if dir == "/" {
            format!("/{name}")
        } else {
            format!("{dir}/{name}")
        };
        delete_entry(writer, &child)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext4_path_from_root() {
        let root = Path::new("/tmp/diff");
        assert_eq!(
            to_ext4_path(root, Path::new("/tmp/diff/usr/bin/foo")),
            "/usr/bin/foo"
        );
        assert_eq!(to_ext4_path(root, Path::new("/tmp/diff/etc")), "/etc");
    }

    #[test]
    fn parent_of_cases() {
        assert_eq!(parent_of("/a/b/c"), "/a/b");
        assert_eq!(parent_of("/a"), "/");
        assert_eq!(parent_of("/"), "/");
    }

    #[test]
    fn whiteout_name_parsing() {
        assert_eq!(".wh.resolv.conf".strip_prefix(".wh."), Some("resolv.conf"));
        assert_eq!("passwd".strip_prefix(".wh."), None);
    }
}
