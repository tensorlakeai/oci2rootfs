//! Post-format customization of the ext4 superblock.
//!
//! `arcbox-ext4`'s [`Formatter`](arcbox_ext4::Formatter) does not expose
//! setters for the superblock's `uuid` or `volume_name` fields. Both are
//! plain byte spans in a layout fixed by the ext2/3/4 on-disk format, so we
//! overwrite them in place after [`Formatter::close`](arcbox_ext4::Formatter::close)
//! completes.
//!
//! The superblock sits at file offset 1024; within it:
//!
//! | Offset | Bytes | Field        |
//! |--------|-------|--------------|
//! | 0x38   | 2     | magic `0xEF53` |
//! | 0x68   | 16    | `uuid`       |
//! | 0x78   | 16    | `volume_name` (NUL-padded ASCII) |
//!
//! Before writing, we read back the magic and refuse to touch the file if
//! it does not look like an ext4 superblock — this catches any future
//! `arcbox-ext4` layout shifts loudly instead of silently corrupting output.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use uuid::Uuid;

use crate::error::{Error, Result};

/// Byte offset of the primary ext4 superblock inside the image file.
const SUPERBLOCK_OFFSET: u64 = 1024;

/// Offsets within the superblock (matching the ext4 on-disk layout).
const MAGIC_OFFSET: u64 = 0x38;
const UUID_OFFSET: u64 = 0x68;
const VOLUME_NAME_OFFSET: u64 = 0x78;
const VOLUME_NAME_LEN: usize = 16;
const SUPERBLOCK_MAGIC: u16 = 0xEF53;

/// Post-format overrides for ext4 image metadata.
///
/// Other properties of the ext4 image (4 KiB block size, no journal,
/// `SPARSE_SUPER2 + EXT_ATTR` feature flags) are fixed by `arcbox-ext4`
/// and not currently configurable.
///
/// ```no_run
/// # use oci2rootfs::{Converter, Ext4Options};
/// use uuid::Uuid;
///
/// let opts = Ext4Options::new()
///     .label("alpine-boot")
///     .uuid(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap());
///
/// Converter::new("rootfs.ext4").ext4_options(opts);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Ext4Options {
    label: Option<String>,
    uuid: Option<Uuid>,
}

impl Ext4Options {
    /// Create an empty options set. Equivalent to [`Default::default`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the filesystem volume label. The ext4 superblock stores the
    /// label in a fixed 16-byte field, so the argument must be ≤ 16 bytes
    /// when UTF-8 encoded; otherwise [`Converter::convert`](crate::Converter::convert)
    /// returns [`Error::InvalidLabel`](crate::Error::InvalidLabel).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set an explicit filesystem UUID. When omitted, `arcbox-ext4` picks a
    /// random v4 UUID at format time.
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    /// `true` when no overrides are set — callers can skip the post-write
    /// entirely in that case.
    pub(crate) fn is_noop(&self) -> bool {
        self.label.is_none() && self.uuid.is_none()
    }
}

/// Apply the overrides by rewriting bytes in the superblock of `path`.
///
/// Validates the superblock magic before writing; returns
/// [`Error::UnsupportedFormat`] when the magic does not match, which
/// signals either a corrupt image or an upstream layout change.
pub(crate) fn apply(path: &Path, opts: &Ext4Options) -> Result<()> {
    if opts.is_noop() {
        return Ok(());
    }

    // Validate label up-front so we don't touch the file on bad input.
    let label_bytes: Option<[u8; VOLUME_NAME_LEN]> = match opts.label.as_deref() {
        Some(label) => Some(encode_label(label)?),
        None => None,
    };

    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    let mut magic = [0u8; 2];
    file.seek(SeekFrom::Start(SUPERBLOCK_OFFSET + MAGIC_OFFSET))?;
    file.read_exact(&mut magic)?;
    if u16::from_le_bytes(magic) != SUPERBLOCK_MAGIC {
        return Err(Error::UnsupportedFormat(format!(
            "{}: ext4 superblock magic mismatch ({:#06x}), refusing to overwrite",
            path.display(),
            u16::from_le_bytes(magic)
        )));
    }

    if let Some(uuid) = opts.uuid {
        file.seek(SeekFrom::Start(SUPERBLOCK_OFFSET + UUID_OFFSET))?;
        file.write_all(uuid.as_bytes())?;
    }

    if let Some(bytes) = label_bytes {
        file.seek(SeekFrom::Start(SUPERBLOCK_OFFSET + VOLUME_NAME_OFFSET))?;
        file.write_all(&bytes)?;
    }

    file.flush()?;
    Ok(())
}

fn encode_label(label: &str) -> Result<[u8; VOLUME_NAME_LEN]> {
    let bytes = label.as_bytes();
    if bytes.len() > VOLUME_NAME_LEN {
        return Err(Error::InvalidLabel(format!(
            "label {label:?} is {} bytes, ext4 limit is {VOLUME_NAME_LEN}",
            bytes.len()
        )));
    }
    if bytes.contains(&0) {
        return Err(Error::InvalidLabel(format!(
            "label {label:?} contains a NUL byte"
        )));
    }
    let mut out = [0u8; VOLUME_NAME_LEN];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_label_fits_in_field() {
        assert_eq!(&encode_label("root").unwrap()[..4], b"root");
    }

    #[test]
    fn encode_label_rejects_oversize() {
        let err = encode_label("this-label-is-way-too-long").unwrap_err();
        assert!(matches!(err, Error::InvalidLabel(_)));
    }

    #[test]
    fn encode_label_rejects_nul() {
        let err = encode_label("root\0label").unwrap_err();
        assert!(matches!(err, Error::InvalidLabel(_)));
    }

    #[test]
    fn noop_skips_when_unset() {
        let opts = Ext4Options::new();
        assert!(opts.is_noop());
    }
}
