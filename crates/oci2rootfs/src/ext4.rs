use std::io::{self, Read};
use std::path::Path;

use ext4_lwext4::{mkfs, Ext4Fs, FileBlockDevice, MkfsOptions, OpenFlags};

use crate::error::Result;

/// High-level ext4 image writer that wraps ext4-lwext4.
pub struct Ext4Writer {
    fs: Ext4Fs,
}

impl Ext4Writer {
    /// Create a new ext4 image file, format it, and mount it for writing.
    pub fn create(path: impl AsRef<Path>, size: u64) -> Result<Self> {
        let path = path.as_ref();

        // Create the disk image file
        let device = FileBlockDevice::create(path, size)?;

        // Format as ext4
        mkfs(device, &MkfsOptions::default())?;

        // Reopen and mount for writing
        let device = FileBlockDevice::open(path)?;
        let fs = Ext4Fs::mount(device, false)?;

        Ok(Self { fs })
    }

    /// Create a directory, creating parent directories as needed.
    /// ext4_dir_mk already creates intermediate directories.
    pub fn mkdir_p(&self, path: &str, mode: u32) -> Result<()> {
        if !self.fs.is_dir(path) {
            self.fs.mkdir(path, mode)?;
        } else {
            // Directory already exists (auto-created by a previous entry),
            // update its permissions.
            self.fs.set_permissions(path, mode)?;
        }
        Ok(())
    }

    /// Write a file from a reader, creating parent directories as needed.
    pub fn write_file(
        &self,
        path: &str,
        reader: &mut dyn Read,
        mode: u32,
        uid: u32,
        gid: u32,
    ) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = parent_path(path) {
            if !self.fs.is_dir(&parent) {
                self.fs.mkdir(&parent, 0o755)?;
            }
        }

        // If the file already exists, remove it first (layer override)
        if self.fs.is_file(path) {
            self.fs.remove(path)?;
        }

        // Create and write the file
        {
            let mut file = self.fs.open(path, OpenFlags::CREATE | OpenFlags::WRITE)?;
            io::copy(reader, &mut file)?;
        }

        // Set metadata
        self.fs.set_permissions(path, mode)?;
        self.fs.set_owner(path, uid, gid)?;

        Ok(())
    }

    /// Create a symbolic link.
    pub fn symlink(&self, target: &str, path: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = parent_path(path) {
            if !self.fs.is_dir(&parent) {
                self.fs.mkdir(&parent, 0o755)?;
            }
        }

        // Remove existing entry if present
        if self.fs.exists(path) {
            self.remove_any(path)?;
        }

        self.fs.symlink(target, path)?;
        Ok(())
    }

    /// Create a hard link.
    pub fn link(&self, src: &str, dst: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = parent_path(dst) {
            if !self.fs.is_dir(&parent) {
                self.fs.mkdir(&parent, 0o755)?;
            }
        }

        // Remove existing entry if present
        if self.fs.exists(dst) {
            self.remove_any(dst)?;
        }

        self.fs.link(src, dst)?;
        Ok(())
    }

    /// Remove a file.
    pub fn remove(&self, path: &str) -> Result<()> {
        if self.fs.is_file(path) {
            self.fs.remove(path)?;
        }
        Ok(())
    }

    /// Remove a directory recursively.
    pub fn rmdir(&self, path: &str) -> Result<()> {
        if self.fs.is_dir(path) {
            self.fs.rmdir(path)?;
        }
        Ok(())
    }

    /// Remove any entry (file or directory).
    fn remove_any(&self, path: &str) -> Result<()> {
        if self.fs.is_dir(path) {
            self.fs.rmdir(path)?;
        } else {
            self.fs.remove(path)?;
        }
        Ok(())
    }

    /// List directory entries (excluding "." and "..").
    pub fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let dir = self.fs.open_dir(path)?;
        let mut entries = Vec::new();
        for entry in dir {
            let entry = entry?;
            let name = entry.name().to_string();
            if name != "." && name != ".." {
                entries.push(name);
            }
        }
        Ok(entries)
    }

    /// Set permissions on a path.
    pub fn set_permissions(&self, path: &str, mode: u32) -> Result<()> {
        self.fs.set_permissions(path, mode)?;
        Ok(())
    }

    /// Set ownership on a path.
    pub fn set_owner(&self, path: &str, uid: u32, gid: u32) -> Result<()> {
        self.fs.set_owner(path, uid, gid)?;
        Ok(())
    }

    /// Check if a path exists.
    pub fn exists(&self, path: &str) -> bool {
        self.fs.exists(path)
    }

    /// Check if a path is a directory.
    pub fn is_dir(&self, path: &str) -> bool {
        self.fs.is_dir(path)
    }

    /// Unmount and finalize the ext4 image.
    pub fn finish(self) -> Result<()> {
        self.fs.umount()?;
        Ok(())
    }
}

/// Extract the parent path from an absolute path.
/// e.g., "/usr/bin/foo" -> Some("/usr/bin")
fn parent_path(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    if let Some(pos) = path.rfind('/') {
        if pos == 0 {
            Some("/".to_string())
        } else {
            Some(path[..pos].to_string())
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_path_root_child() {
        assert_eq!(parent_path("/foo"), Some("/".to_string()));
    }

    #[test]
    fn test_parent_path_nested() {
        assert_eq!(parent_path("/usr/bin/foo"), Some("/usr/bin".to_string()));
    }

    #[test]
    fn test_parent_path_trailing_slash() {
        assert_eq!(parent_path("/usr/bin/"), Some("/usr".to_string()));
    }

    #[test]
    fn test_parent_path_no_slash() {
        assert_eq!(parent_path("foo"), None);
    }

    #[test]
    fn test_parent_path_root() {
        // "/" after trim_end_matches('/') becomes "", rfind('/') returns None
        assert_eq!(parent_path("/"), None);
    }
}
