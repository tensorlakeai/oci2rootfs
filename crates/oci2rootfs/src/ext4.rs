use std::io::Read;
use std::path::Path;

use arcbox_ext4::Formatter;
use arcbox_ext4::constants::{file_mode, make_mode};

use crate::error::Result;
use crate::path::join;

/// High-level ext4 image writer that wraps arcbox-ext4's Formatter.
pub struct Ext4Writer {
    formatter: Formatter,
}

impl Ext4Writer {
    /// Create a new ext4 image file, format it, and prepare for writing.
    pub fn create(path: impl AsRef<Path>, size: u64) -> Result<Self> {
        let formatter = Formatter::new(path.as_ref(), 4096, size)?;
        Ok(Self { formatter })
    }

    /// Create a directory, creating parent directories as needed.
    pub fn mkdir_p(&mut self, path: &str, mode: u32) -> Result<()> {
        let perm = (mode & 0o7777) as u16;
        if !self.formatter.is_dir(path) {
            self.formatter.create(
                path,
                make_mode(file_mode::S_IFDIR, perm),
                None,
                None,
                None,
                None,
                None,
                None,
            )?;
        } else {
            // Directory already exists -- update permissions.
            self.formatter.set_permissions(path, perm)?;
        }
        Ok(())
    }

    /// Write a file from a reader, creating parent directories as needed.
    pub fn write_file(
        &mut self,
        path: &str,
        reader: &mut dyn Read,
        mode: u32,
        uid: u32,
        gid: u32,
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
            None,
            Some(reader),
            Some(uid),
            Some(gid),
            None,
        )?;
        Ok(())
    }

    /// Create a symbolic link at `path` pointing to `target`.
    pub fn symlink(&mut self, target: &str, path: &str) -> Result<()> {
        if self.formatter.exists(path) {
            self.delete(path)?;
        }

        self.formatter.create(
            path,
            make_mode(file_mode::S_IFLNK, 0o777),
            Some(target),
            None,
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

    /// Set ownership on a path.
    pub fn set_owner(&mut self, path: &str, uid: u32, gid: u32) -> Result<()> {
        self.formatter.set_owner(path, uid, gid)?;
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
        let writer = Ext4Writer::create(file.path(), TEST_SIZE).unwrap();
        (writer, file)
    }

    #[test]
    #[serial]
    fn mkdir_p_updates_permissions_on_existing_dir() {
        let (mut writer, _file) = create_writer();
        writer.mkdir_p("/etc", 0o755).unwrap();
        writer.mkdir_p("/etc", 0o700).unwrap();
        assert!(writer.is_dir("/etc"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn write_file_overwrites_existing_file() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file("/f", &mut Cursor::new(b"first"), 0o644, 0, 0)
            .unwrap();
        writer
            .write_file("/f", &mut Cursor::new(b"second"), 0o644, 0, 0)
            .unwrap();
        assert!(writer.exists("/f"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn write_file_overwrites_existing_symlink() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file("/target", &mut Cursor::new(b"t"), 0o644, 0, 0)
            .unwrap();
        writer.symlink("/target", "/link").unwrap();
        // Layer-override: a tar entry of type Regular shadows the symlink.
        writer
            .write_file("/link", &mut Cursor::new(b"plain"), 0o644, 0, 0)
            .unwrap();
        assert!(writer.exists("/link"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn delete_recursively_removes_nested_directory() {
        let (mut writer, _file) = create_writer();
        writer.mkdir_p("/a", 0o755).unwrap();
        writer.mkdir_p("/a/b", 0o755).unwrap();
        writer
            .write_file("/a/b/c.txt", &mut Cursor::new(b"c"), 0o644, 0, 0)
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
        writer.mkdir_p("/cache", 0o755).unwrap();
        writer
            .write_file("/cache/a", &mut Cursor::new(b"a"), 0o644, 0, 0)
            .unwrap();
        writer.mkdir_p("/cache/sub", 0o755).unwrap();
        writer
            .write_file("/cache/sub/b", &mut Cursor::new(b"b"), 0o644, 0, 0)
            .unwrap();

        writer.clear_dir("/cache").unwrap();

        assert!(writer.is_dir("/cache"));
        assert!(!writer.exists("/cache/a"));
        assert!(!writer.exists("/cache/sub"));
        writer.finish().unwrap();
    }
}
