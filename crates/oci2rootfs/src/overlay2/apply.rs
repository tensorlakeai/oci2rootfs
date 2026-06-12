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
use crate::path::{Whiteout, join, parent_of, parse_oci_whiteout};

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

        // OCI whiteouts (.wh.<name>, .wh..wh..opq).
        if let Some(marker) = parse_oci_whiteout(&name_str) {
            let parent = parent_of(&ext4_path);
            match marker {
                Whiteout::Delete(target) => writer.delete(&join(&parent, target))?,
                Whiteout::Opaque => writer.clear_dir(&parent)?,
            }
            continue;
        }

        // Overlay2-native whiteout: character device with rdev == 0.
        if is_chardev_whiteout(&meta) {
            writer.delete(&ext4_path)?;
            continue;
        }

        if meta.is_symlink() {
            let target = fs::read_link(&full_path)?;
            writer.symlink(&target.to_string_lossy(), &ext4_path)?;
        } else if meta.is_dir() {
            let (mode, uid, gid) = ownership(&meta);
            let xattrs = host_xattrs(&full_path)?;
            writer.mkdir_p_with_metadata(
                &ext4_path,
                mode,
                Some(uid),
                Some(gid),
                optional_xattrs(&xattrs),
            )?;
            walk(root, &full_path, writer, hardlinks)?;
        } else if meta.is_file() {
            // Hardlink detection: if we've already written a file with the
            // same (dev, ino), create a hardlink instead of a duplicate copy.
            if let Some(existing) = check_hardlink(&meta, hardlinks, &ext4_path) {
                writer.link(&existing, &ext4_path)?;
            } else {
                let (mode, uid, gid) = ownership(&meta);
                let xattrs = host_xattrs(&full_path)?;
                let mut file = fs::File::open(&full_path)?;
                writer.write_file_with_xattrs(
                    &ext4_path,
                    &mut file,
                    mode,
                    uid,
                    gid,
                    optional_xattrs(&xattrs),
                )?;
            }
        }
        // Skip sockets, FIFOs, and non-whiteout device nodes.
    }

    Ok(())
}

fn host_xattrs(path: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let mut xattrs = HashMap::new();
    for name in xattr::list(path)? {
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if let Some(value) = xattr::get(path, &name)? {
            xattrs.insert(name_str.to_string(), value);
        }
    }
    Ok(xattrs)
}

fn optional_xattrs(xattrs: &HashMap<String, Vec<u8>>) -> Option<&HashMap<String, Vec<u8>>> {
    (!xattrs.is_empty()).then_some(xattrs)
}

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
    }
}
