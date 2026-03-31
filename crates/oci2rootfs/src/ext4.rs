use std::io::Read;
use std::path::Path;

use arcbox_ext4::constants::{file_mode, make_mode};
use arcbox_ext4::Formatter;

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
            self.formatter
                .create(path, make_mode(file_mode::S_IFDIR, perm), None, None, None, None, None, None)?;
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

    /// Set permissions on a path.
    pub fn set_permissions(&mut self, path: &str, mode: u32) -> Result<()> {
        let perm = (mode & 0o7777) as u16;
        self.formatter.set_permissions(path, perm)?;
        Ok(())
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

