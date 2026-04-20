use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ext4 format error: {0}")]
    Ext4Format(#[from] arcbox_ext4::error::FormatError),

    #[error("layout error: {0}")]
    Layout(#[from] containerregistry_layout::Error),

    #[error("image error: {0}")]
    Image(#[from] containerregistry_image::Error),

    #[cfg(feature = "remote")]
    #[error("registry error: {0}")]
    Registry(#[from] containerregistry_registry::Error),

    #[cfg(feature = "remote")]
    #[error("authentication error: {0}")]
    Auth(#[from] containerregistry_auth::Error),

    #[error("no matching manifest found for platform {0}")]
    NoManifest(String),

    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[cfg(feature = "remote")]
    #[error("invalid reference: {0}")]
    InvalidReference(String),

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("invalid tar entry path: {0}")]
    InvalidTarPath(String),
}

pub type Result<T> = std::result::Result<T, Error>;
