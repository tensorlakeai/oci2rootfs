//! Superblock metadata overrides for the output ext4 image.
//!
//! `Ext4Options` is a thin data holder carried on [`crate::Converter`]; the
//! actual write happens in [`arcbox_ext4::Formatter::with_options`] when the
//! image is formatted. Label validation (≤ 16 bytes, NUL-free) lives in
//! `arcbox-ext4` and surfaces here as
//! [`Error::Ext4Format`](crate::Error::Ext4Format) wrapping
//! [`arcbox_ext4::error::FormatError::InvalidLabel`].

use uuid::Uuid;

/// Caller-supplied overrides for the output ext4 superblock.
///
/// Other properties of the ext4 image (4 KiB block size, no journal,
/// `SPARSE_SUPER2 | EXT_ATTR` feature flags) are fixed by `arcbox-ext4`
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
    pub(crate) label: Option<String>,
    pub(crate) uuid: Option<Uuid>,
}

impl Ext4Options {
    /// Create an empty options set. Equivalent to [`Default::default`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the filesystem volume label. Must fit in the ext4 superblock's
    /// 16-byte `volume_name` field; validation is enforced by `arcbox-ext4`
    /// at format time.
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
}
