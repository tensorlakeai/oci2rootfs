use thiserror::Error;

/// Errors produced during image resolution, layer application, and ext4
/// image writing.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error from the host filesystem — reading an OCI layout blob,
    /// walking an overlay2 `diff/` tree, or writing the ext4 image file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The ext4 formatter rejected an operation (for example, insufficient
    /// space in the output image, or an inode-table exhaustion).
    #[error("ext4 format error: {0}")]
    Ext4Format(#[from] arcbox_ext4::error::FormatError),

    /// The OCI image layout on disk was malformed: missing `oci-layout` /
    /// `index.json`, corrupt `blobs/sha256/*`, or an unreadable blob.
    #[error("layout error: {0}")]
    Layout(#[from] containerregistry_layout::Error),

    /// An OCI image manifest, index, or config could not be parsed.
    #[error("image error: {0}")]
    Image(#[from] containerregistry_image::Error),

    /// The remote registry returned an error response or the request could
    /// not be completed (network failure, auth challenge, 4xx/5xx).
    #[cfg(feature = "remote")]
    #[error("registry error: {0}")]
    Registry(#[from] containerregistry_registry::Error),

    /// Docker-config credential resolution failed — typically a malformed
    /// `~/.docker/config.json` or a missing credential helper binary.
    #[cfg(feature = "remote")]
    #[error("authentication error: {0}")]
    Auth(#[from] containerregistry_auth::Error),

    /// No manifest in the image index matched the requested platform and
    /// no fallback descriptor was available.
    #[error("no matching manifest found for platform {0}")]
    NoManifest(String),

    /// A layer, manifest, or config descriptor carried a media type the
    /// crate does not know how to consume.
    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),

    /// The image reference string did not parse as a valid OCI reference.
    #[cfg(feature = "remote")]
    #[error("invalid reference: {0}")]
    InvalidReference(String),

    /// The source path was neither a recognized OCI image layout nor a
    /// Docker overlay2 layer directory.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// A tar entry's path, symlink target, or hardlink target contained a
    /// parent-dir (`..`) component or a NUL byte, and was rejected before
    /// any data was written to the ext4 image.
    #[error("invalid tar entry path: {0}")]
    InvalidTarPath(String),

    /// The configured output size is smaller than the estimated raw content
    /// size for the source. The preflight check fires before the output file
    /// is created, so no partial image is left behind.
    #[error("output size too small: need at least {needed} bytes, got {configured}")]
    InsufficientSize {
        /// Estimated lower bound on the raw bytes needed to fit the image.
        needed: u64,
        /// Output size as configured on the [`crate::Converter`].
        configured: u64,
    },

    /// A volume label supplied via [`crate::Ext4Options`] did not fit in the
    /// superblock's 16-byte `volume_name` field (ext4 hard limit).
    #[error("invalid ext4 label: {0}")]
    InvalidLabel(String),
}

/// Short-hand for [`std::result::Result`] specialized to [`enum@Error`].
pub type Result<T> = std::result::Result<T, Error>;
