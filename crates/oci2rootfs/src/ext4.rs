use std::io::Read;
use std::path::Path;

use arcbox_ext4::Formatter;
use arcbox_ext4::constants::{file_mode, make_mode};

use crate::error::Result;

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

        // If the file already exists, remove it first (layer override).
        if self.formatter.exists(path) && !self.formatter.is_dir(path) {
            self.formatter.unlink(path, false)?;
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

    /// Create a symbolic link.
    pub fn symlink(&mut self, target: &str, path: &str) -> Result<()> {
        // Remove existing entry if present.
        if self.formatter.exists(path) {
            self.formatter.unlink(path, false)?;
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

    /// Create a hard link.
    pub fn link(&mut self, src: &str, dst: &str) -> Result<()> {
        // Remove existing entry if present.
        if self.formatter.exists(dst) {
            self.formatter.unlink(dst, false)?;
        }

        self.formatter.link(dst, src)?;
        Ok(())
    }

    /// Remove a file.
    pub fn remove(&mut self, path: &str) -> Result<()> {
        if self.formatter.exists(path) && !self.formatter.is_dir(path) {
            self.formatter.unlink(path, false)?;
        }
        Ok(())
    }

    /// Remove a directory and its contents.
    pub fn rmdir(&mut self, path: &str) -> Result<()> {
        if self.formatter.is_dir(path) {
            self.formatter.unlink(path, false)?;
        }
        Ok(())
    }

    /// List directory entries (excluding "." and "..").
    pub fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        Ok(self.formatter.list_dir(path))
    }

    /// Set ownership on a path.
    pub fn set_owner(&mut self, path: &str, uid: u32, gid: u32) -> Result<()> {
        self.formatter.set_owner(path, uid, gid)?;
        Ok(())
    }

    /// Check if a path exists.
    pub fn exists(&self, path: &str) -> bool {
        self.formatter.exists(path)
    }

    /// Check if a path is a directory.
    pub fn is_dir(&self, path: &str) -> bool {
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
    fn create_and_finish() {
        let (writer, file) = create_writer();
        writer.finish().unwrap();
        let metadata = std::fs::metadata(file.path()).unwrap();
        assert_eq!(metadata.len(), TEST_SIZE);
    }

    #[test]
    #[serial]
    fn mkdir_and_exists() {
        let (mut writer, _file) = create_writer();
        writer.mkdir_p("/testdir", 0o755).unwrap();
        assert!(writer.exists("/testdir"));
        assert!(writer.is_dir("/testdir"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn mkdir_idempotent() {
        let (mut writer, _file) = create_writer();
        writer.mkdir_p("/testdir", 0o755).unwrap();
        writer.mkdir_p("/testdir", 0o700).unwrap();
        assert!(writer.is_dir("/testdir"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn write_file() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file("/hello.txt", &mut Cursor::new(b"hello world"), 0o644, 0, 0)
            .unwrap();
        assert!(writer.exists("/hello.txt"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn write_file_overwrite() {
        let (mut writer, _file) = create_writer();
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
    fn symlink() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file("/target.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
            .unwrap();
        writer.symlink("/target.txt", "/link.txt").unwrap();
        assert!(writer.exists("/link.txt"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn link() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file("/original.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
            .unwrap();
        writer.link("/original.txt", "/hardlink.txt").unwrap();
        assert!(writer.exists("/hardlink.txt"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn remove() {
        let (mut writer, _file) = create_writer();
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
    fn rmdir() {
        let (mut writer, _file) = create_writer();
        writer.mkdir_p("/emptydir", 0o755).unwrap();
        assert!(writer.is_dir("/emptydir"));
        writer.rmdir("/emptydir").unwrap();
        assert!(!writer.is_dir("/emptydir"));
        writer.finish().unwrap();
    }

    #[test]
    #[serial]
    fn list_dir() {
        let (mut writer, _file) = create_writer();
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
    fn set_owner() {
        let (mut writer, _file) = create_writer();
        writer
            .write_file("/perm.txt", &mut Cursor::new(b"data"), 0o644, 0, 0)
            .unwrap();
        writer.set_owner("/perm.txt", 1000, 1000).unwrap();
        writer.finish().unwrap();
    }
}
