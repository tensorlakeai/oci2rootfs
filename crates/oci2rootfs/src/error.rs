use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ext4 format error: {0}")]
    Ext4Format(#[from] arcbox_ext4::error::FormatError),

    #[error("layout error: {0}")]
    Layout(#[from] containerregistry::layout::Error),

    #[error("image error: {0}")]
    Image(#[from] containerregistry::image::Error),

    #[error("registry error: {0}")]
    Registry(#[from] containerregistry::registry::Error),

    #[error("authentication error: {0}")]
    Auth(#[from] containerregistry::auth::Error),

    #[error("no matching manifest found for platform {0}")]
    NoManifest(String),

    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[error("invalid reference: {0}")]
    InvalidReference(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_manifest_display() {
        let err = Error::NoManifest("linux/amd64".into());
        assert_eq!(
            err.to_string(),
            "no matching manifest found for platform linux/amd64"
        );
    }

    #[test]
    fn test_unsupported_media_type_display() {
        let err = Error::UnsupportedMediaType("application/vnd.unknown".into());
        assert_eq!(
            err.to_string(),
            "unsupported media type: application/vnd.unknown"
        );
    }

    #[test]
    fn test_invalid_reference_display() {
        let err = Error::InvalidReference("bad ref".into());
        assert_eq!(err.to_string(), "invalid reference: bad ref");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("file not found"));
    }
}
