//! Docker overlay2 storage directory support.
//!
//! Docker's overlay2 driver stores image layers as:
//!
//! ```text
//! /var/lib/docker/overlay2/
//! ├── l/                        # Symlinks: <short-id> → ../<chain-id>/diff
//! ├── <chain-id>/
//! │   ├── diff/                 # Layer filesystem content
//! │   ├── link                  # Short ID for this layer
//! │   └── lower                 # Colon-separated lower layer references
//! └── ...
//! ```
//!
//! This module resolves the full layer chain from a topmost layer directory,
//! then applies each layer's `diff/` tree directly to an ext4 filesystem —
//! no tar intermediate needed.

mod apply;

use std::fs;
use std::path::{Path, PathBuf};

use crate::convert::SourceImpl;
use crate::error::{Error, Result};
use crate::ext4::Ext4Writer;

/// Resolved overlay2 layer chain, ready for ext4 application.
pub(crate) struct Overlay2Archive {
    /// Layer diff directories ordered bottom-to-top (base layer first).
    layers: Vec<PathBuf>,
}

/// Check whether `path` looks like an overlay2 layer directory.
///
/// Returns `true` when the directory contains both `diff/` and `link`,
/// which are characteristic of Docker's overlay2 storage.
pub(crate) fn is_overlay2(path: &Path) -> bool {
    path.is_dir() && path.join("diff").is_dir() && path.join("link").is_file()
}

/// Resolve an overlay2 layer directory into an ordered layer chain.
///
/// `path` must be the chain-id directory (e.g.
/// `/var/lib/docker/overlay2/<chain-id>`). The function reads `lower`
/// references to discover all layers and returns them bottom-to-top.
pub(crate) fn resolve(path: &Path) -> Result<Overlay2Archive> {
    let diff = path.join("diff");
    if !diff.is_dir() {
        return Err(Error::UnsupportedFormat(format!(
            "{}: not an overlay2 layer (missing diff/ directory)",
            path.display()
        )));
    }

    if !path.join("link").is_file() {
        return Err(Error::UnsupportedFormat(format!(
            "{}: not an overlay2 layer (missing link file)",
            path.display()
        )));
    }

    let overlay2_root = path
        .parent()
        .ok_or_else(|| Error::UnsupportedFormat("overlay2 layer has no parent directory".into()))?;

    // Collect lower layers by following the `lower` file.
    let mut layers = Vec::new();
    collect_lower_layers(path, overlay2_root, &mut layers)?;

    // `lower` lists nearest-first; reverse so base layer comes first.
    layers.reverse();

    // Append the topmost layer.
    layers.push(diff);

    Ok(Overlay2Archive { layers })
}

/// Follow `lower` references to collect all diff directories.
fn collect_lower_layers(
    layer_dir: &Path,
    overlay2_root: &Path,
    layers: &mut Vec<PathBuf>,
) -> Result<()> {
    let lower_file = layer_dir.join("lower");
    if !lower_file.exists() {
        return Ok(()); // base layer — no further references
    }

    let canonical_root = overlay2_root.canonicalize()?;
    let content = fs::read_to_string(&lower_file)?;
    for link_ref in content.trim().split(':') {
        if link_ref.is_empty() {
            continue;
        }
        // `lower` entries are like `l/ABC123`, relative to overlay2_root.
        let link_path = overlay2_root.join(link_ref);
        let diff_path = if link_path.is_symlink() {
            let target = fs::read_link(&link_path)?;
            if target.is_absolute() {
                target
            } else {
                link_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(&target)
                    .canonicalize()?
            }
        } else {
            link_path.canonicalize()?
        };

        // Reject symlinks that escape the overlay2 storage directory.
        if !diff_path.starts_with(&canonical_root) {
            return Err(Error::UnsupportedFormat(format!(
                "lower layer reference escapes overlay2 root: {}",
                diff_path.display()
            )));
        }

        layers.push(diff_path);
    }

    Ok(())
}

impl SourceImpl for Overlay2Archive {
    fn layer_count(&self) -> usize {
        self.layers.len()
    }

    fn config(&self) -> Option<&containerregistry_image::ImageConfig> {
        // Docker stores the image config outside the overlay2 layer
        // directory (under `/var/lib/docker/image/overlay2/imagedb/`),
        // which this crate does not read.
        None
    }

    fn estimated_raw_size(&self) -> Option<u64> {
        let mut total: u64 = 0;
        for diff in &self.layers {
            total = total.saturating_add(tree_size(diff).ok()?);
        }
        Some(total.saturating_add(16 * 1024 * 1024))
    }

    fn apply_to(&self, writer: &mut Ext4Writer) -> Result<()> {
        for (index, diff_dir) in self.layers.iter().enumerate() {
            let span = tracing::info_span!(
                "apply_overlay2_layer",
                layer_index = index + 1,
                layer_count = self.layers.len(),
                diff = %diff_dir.display(),
            );
            let _guard = span.enter();
            apply::apply_directory_layer(diff_dir, writer)?;
        }
        Ok(())
    }
}

/// Recursively sum sizes of regular files under `root` using
/// `symlink_metadata().len()` (symlinks and directories contribute nothing).
fn tree_size(root: &Path) -> std::io::Result<u64> {
    let mut stack = vec![root.to_path_buf()];
    let mut total: u64 = 0;
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detect_overlay2_directory() {
        let dir = TempDir::new().unwrap();
        assert!(!is_overlay2(dir.path()));

        fs::create_dir(dir.path().join("diff")).unwrap();
        assert!(!is_overlay2(dir.path()));

        fs::write(dir.path().join("link"), "ABCD").unwrap();
        assert!(is_overlay2(dir.path()));
    }

    #[test]
    fn resolve_single_layer() {
        let root = TempDir::new().unwrap();
        let layer = root.path().join("layer-a");
        fs::create_dir_all(layer.join("diff")).unwrap();
        fs::write(layer.join("link"), "AAA").unwrap();

        let archive = resolve(&layer).unwrap();
        assert_eq!(archive.layers.len(), 1);
        assert_eq!(archive.layers[0], layer.join("diff"));
    }

    #[test]
    #[cfg(unix)]
    fn resolve_multi_layer_chain() {
        let root = TempDir::new().unwrap();

        // Base layer.
        let base = root.path().join("base");
        fs::create_dir_all(base.join("diff")).unwrap();
        fs::write(base.join("link"), "BASE").unwrap();

        // Symlink directory.
        let l_dir = root.path().join("l");
        fs::create_dir(&l_dir).unwrap();
        std::os::unix::fs::symlink("../base/diff", l_dir.join("BASE")).unwrap();

        // Top layer referencing base.
        let top = root.path().join("top");
        fs::create_dir_all(top.join("diff")).unwrap();
        fs::write(top.join("link"), "TOP").unwrap();
        fs::write(top.join("lower"), "l/BASE").unwrap();

        let archive = resolve(&top).unwrap();
        assert_eq!(archive.layers.len(), 2);
        assert_eq!(
            fs::canonicalize(&archive.layers[0]).unwrap(),
            fs::canonicalize(base.join("diff")).unwrap()
        );
        assert_eq!(archive.layers[1], top.join("diff"));
    }
}
