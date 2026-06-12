use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use arcbox_ext4::constants::{file_mode, make_mode};
use arcbox_ext4::{FileTimestamps, FormatOptions, Formatter};

use crate::error::Result;
use crate::ext4_options::Ext4Options;
use crate::path::join;

/// High-level ext4 image writer that wraps arcbox-ext4's Formatter.
pub struct Ext4Writer {
    formatter: Formatter,
}

pub(crate) fn file_timestamps_from_unix_secs(seconds: u64) -> FileTimestamps {
    let mut timestamps = FileTimestamps::default();
    let seconds_lo = seconds as u32;
    timestamps.access_lo = seconds_lo;
    timestamps.access_hi = 0;
    timestamps.modification_lo = seconds_lo;
    timestamps.modification_hi = 0;
    timestamps.creation_lo = seconds_lo;
    timestamps.creation_hi = 0;
    timestamps
}

impl Ext4Writer {
    /// Create a new ext4 image file with caller-supplied superblock overrides.
    ///
    /// `opts` surfaces the subset of superblock fields that the bake pipeline
    /// cares about (UUID, label). Other formatter parameters stay fixed at
    /// their arcbox-ext4 defaults.
    pub fn create(path: impl AsRef<Path>, size: u64, opts: &Ext4Options) -> Result<Self> {
        let mut fmt_opts = FormatOptions::new(size);
        if let Some(uuid) = opts.uuid {
            fmt_opts = fmt_opts.uuid(uuid);
        }
        if let Some(label) = &opts.label {
            fmt_opts = fmt_opts.label(label);
        }
        let formatter = Formatter::with_options(path.as_ref(), fmt_opts)?;
        Ok(Self { formatter })
    }

    /// Create a directory with ownership and extended attributes.
    ///
    /// Extended attributes are applied when the directory inode is first
    /// created. If the directory already exists, only mode and ownership are
    /// updated because arcbox-ext4 does not currently expose post-creation
    /// xattr mutation.
    pub fn mkdir_p_with_metadata(
        &mut self,
        path: &str,
        mode: u32,
        uid: Option<u32>,
        gid: Option<u32>,
        timestamps: Option<FileTimestamps>,
        xattrs: Option<&HashMap<String, Vec<u8>>>,
    ) -> Result<()> {
        let perm = (mode & 0o7777) as u16;
        if !self.formatter.is_dir(path) {
            self.formatter.create(
                path,
                make_mode(file_mode::S_IFDIR, perm),
                None,
                timestamps,
                None,
                uid,
                gid,
                xattrs,
            )?;
        } else {
            // Directory already exists -- update metadata we can mutate safely.
            self.formatter.set_permissions(path, perm)?;
            if uid.is_some() || gid.is_some() {
                self.formatter
                    .set_owner(path, uid.unwrap_or(0), gid.unwrap_or(0))?;
            }
        }
        Ok(())
    }

    /// Write a file from a reader with extended attributes, creating parent directories as needed.
    pub fn write_file_with_xattrs(
        &mut self,
        path: &str,
        reader: &mut dyn Read,
        mode: u32,
        uid: u32,
        gid: u32,
        timestamps: Option<FileTimestamps>,
        xattrs: Option<&HashMap<String, Vec<u8>>>,
    ) -> Result<()> {
        let perm = (mode & 0o7777) as u16;

        // If an entry already exists at this path, remove it first (layer
        // override). Directories are removed recursively; anything else is
        // unlinked.
        if self.formatter.exists(path) {
            self.delete(path)?;
        }

        self.formatter.create(
            path,
            make_mode(file_mode::S_IFREG, perm),
            None,
            timestamps,
            Some(reader),
            Some(uid),
            Some(gid),
            xattrs,
        )?;
        Ok(())
    }

    /// Create a symbolic link at `path` pointing to `target`.
    pub fn symlink(
        &mut self,
        target: &str,
        path: &str,
        timestamps: Option<FileTimestamps>,
    ) -> Result<()> {
        if self.formatter.exists(path) {
            self.delete(path)?;
        }

        self.formatter.create(
            path,
            make_mode(file_mode::S_IFLNK, 0o777),
            Some(target),
            timestamps,
            None,
            None,
            None,
            None,
        )?;
        Ok(())
    }

    /// Create a hard link at `path` pointing to `target`.
    pub fn link(&mut self, target: &str, path: &str) -> Result<()> {
        if self.formatter.exists(path) {
            self.delete(path)?;
        }

        self.formatter.link(path, target)?;
        Ok(())
    }

    /// Delete any entry at `path` — file, symlink, or directory (recursively).
    ///
    /// Returns `Ok(())` if the path does not exist; this matches the semantics
    /// of OCI whiteouts, which may target entries that never appeared in a
    /// lower layer.
    pub fn delete(&mut self, path: &str) -> Result<()> {
        if !self.formatter.exists(path) {
            return Ok(());
        }

        if self.formatter.is_dir(path) {
            self.clear_dir(path)?;
        }
        self.formatter.unlink(path, false)?;
        Ok(())
    }

    /// Recursively remove every child of `path`, leaving the directory itself.
    ///
    /// No-op when `path` does not exist or is not a directory.
    pub fn clear_dir(&mut self, path: &str) -> Result<()> {
        if !self.formatter.is_dir(path) {
            return Ok(());
        }
        let entries = self.formatter.list_dir(path);
        for name in entries {
            self.delete(&join(path, &name))?;
        }
        Ok(())
    }

    /// Check if a path exists. Available in tests so assertions can inspect
    /// image state without exposing the underlying `Formatter`.
    #[cfg(test)]
    pub(crate) fn exists(&self, path: &str) -> bool {
        self.formatter.exists(path)
    }

    /// Check if a path is a directory. See [`Self::exists`] for rationale.
    #[cfg(test)]
    pub(crate) fn is_dir(&self, path: &str) -> bool {
        self.formatter.is_dir(path)
    }

    /// Finalize the ext4 image.
    pub fn finish(self) -> Result<()> {
        self.formatter.close()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io::Cursor;
    use tempfile::NamedTempFile;

    const TEST_SIZE: u64 = 16 * 1024 * 1024;

    fn create_writer() -> (Ext4Writer, NamedTempFile) {
        let file = NamedTempFile::new().unwrap();
        let writer = Ext4Writer::create(file.path(), TEST_SIZE, &Ext4Options::default()).unwrap();
        (writer, file)
    }

    #[test]
    #[serial]
    fn mkdir_p_updates_permissions_on_existing_dir() {
        let (mut writer, _file) = create_writer();
        writer
            .mkdir_p_with_metadata("/etc", 0o755, None, None, None, None)
            .unwrap();
        writer
            .mkdir_p_with_metadata("/etc", 0o700, None, None, None, None)
            .unwrap();
        assert!(writer.is_dir("/etc"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn write_file_overwrites_existing_file() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file_with_xattrs("/f", &mut Cursor::new(b"first"), 0o644, 0, 0, None, None)
            .unwrap();
        writer
            .write_file_with_xattrs("/f", &mut Cursor::new(b"second"), 0o644, 0, 0, None, None)
            .unwrap();
        assert!(writer.exists("/f"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn write_file_overwrites_existing_symlink() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file_with_xattrs("/target", &mut Cursor::new(b"t"), 0o644, 0, 0, None, None)
            .unwrap();
        writer.symlink("/target", "/link", None).unwrap();
        // Layer-override: a tar entry of type Regular shadows the symlink.
        writer
            .write_file_with_xattrs("/link", &mut Cursor::new(b"plain"), 0o644, 0, 0, None, None)
            .unwrap();
        assert!(writer.exists("/link"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn delete_recursively_removes_nested_directory() {
        let (mut writer, _file) = create_writer();
        writer
            .mkdir_p_with_metadata("/a", 0o755, None, None, None, None)
            .unwrap();
        writer
            .mkdir_p_with_metadata("/a/b", 0o755, None, None, None, None)
            .unwrap();
        writer
            .write_file_with_xattrs(
                "/a/b/c.txt",
                &mut Cursor::new(b"c"),
                0o644,
                0,
                0,
                None,
                None,
            )
            .unwrap();

        writer.delete("/a").unwrap();

        assert!(!writer.exists("/a"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn delete_missing_path_is_noop() {
        let (mut writer, _file) = create_writer();
        writer.delete("/does/not/exist").unwrap();
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn clear_dir_leaves_directory_in_place() {
        let (mut writer, _file) = create_writer();
        writer
            .mkdir_p_with_metadata("/cache", 0o755, None, None, None, None)
            .unwrap();
        writer
            .write_file_with_xattrs("/cache/a", &mut Cursor::new(b"a"), 0o644, 0, 0, None, None)
            .unwrap();
        writer
            .mkdir_p_with_metadata("/cache/sub", 0o755, None, None, None, None)
            .unwrap();
        writer
            .write_file_with_xattrs(
                "/cache/sub/b",
                &mut Cursor::new(b"b"),
                0o644,
                0,
                0,
                None,
                None,
            )
            .unwrap();

        writer.clear_dir("/cache").unwrap();

        assert!(writer.is_dir("/cache"));
        assert!(!writer.exists("/cache/a"));
        assert!(!writer.exists("/cache/sub"));
        writer.finish().unwrap();
    }
}
