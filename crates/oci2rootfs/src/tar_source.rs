use std::fs::File;
#[cfg(feature = "remote")]
use std::io::Cursor;
use std::io::Read;
use std::path::PathBuf;
#[cfg(feature = "remote")]
use std::sync::Arc;

use containerregistry_image::{Descriptor, ImageConfig, MediaType};
use flate2::read::GzDecoder;

use crate::convert::SourceImpl;
use crate::error::{Error, Result};
use crate::ext4::Ext4Writer;
use crate::layer::apply_layer;

/// A tar-layer-backed image source that can read blobs from memory or disk.
pub(crate) struct TarImageSource {
    config: ImageConfig,
    layers: Vec<TarLayer>,
}

impl TarImageSource {
    /// Build a source whose layer blobs already live in memory.
    #[cfg(feature = "remote")]
    pub(crate) fn from_memory(config: ImageConfig, layers: Vec<(Descriptor, Vec<u8>)>) -> Self {
        let layers = layers
            .into_iter()
            .map(|(descriptor, blob)| TarLayer {
                descriptor,
                opener: Box::new(MemoryBlob::new(blob)),
            })
            .collect();

        Self { config, layers }
    }

    /// Build a source whose layer blobs live on disk.
    pub(crate) fn from_files(config: ImageConfig, layers: Vec<(Descriptor, PathBuf)>) -> Self {
        let layers = layers
            .into_iter()
            .map(|(descriptor, path)| TarLayer {
                descriptor,
                opener: Box::new(FileBlob::new(path)),
            })
            .collect();

        Self { config, layers }
    }

    #[cfg(test)]
    pub(crate) fn config(&self) -> &ImageConfig {
        &self.config
    }
}

impl SourceImpl for TarImageSource {
    fn layer_count(&self) -> usize {
        self.layers.len()
    }

    fn config(&self) -> Option<&ImageConfig> {
        Some(&self.config)
    }

    fn apply_to(&self, writer: &mut Ext4Writer) -> Result<()> {
        for (index, layer) in self.layers.iter().enumerate() {
            let span = tracing::info_span!(
                "apply_layer",
                layer_index = index + 1,
                layer_count = self.layers.len(),
                digest = %layer.descriptor.digest,
                bytes = layer.descriptor.size,
            );
            let _guard = span.enter();
            let reader = layer.open()?;
            apply_layer(reader, writer)?;
        }
        Ok(())
    }
}

struct TarLayer {
    descriptor: Descriptor,
    opener: Box<dyn BlobOpener>,
}

impl TarLayer {
    fn open(&self) -> Result<Box<dyn Read>> {
        let blob = self.opener.open_blob()?;

        match self.descriptor.media_type {
            MediaType::OciLayerGzip
            | MediaType::DockerLayerGzip
            | MediaType::OciLayerNondistributableGzip => Ok(Box::new(GzDecoder::new(blob))),
            MediaType::OciLayerZstd | MediaType::OciLayerNondistributableZstd => {
                Ok(Box::new(zstd::Decoder::new(blob)?))
            }
            MediaType::OciLayer | MediaType::OciLayerNondistributable => Ok(blob),
            ref other => Err(Error::UnsupportedMediaType(other.as_str().to_string())),
        }
    }
}

trait BlobOpener {
    fn open_blob(&self) -> Result<Box<dyn Read>>;
}

#[cfg(feature = "remote")]
struct MemoryBlob {
    bytes: Arc<[u8]>,
}

#[cfg(feature = "remote")]
impl MemoryBlob {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::from(bytes),
        }
    }
}

#[cfg(feature = "remote")]
impl BlobOpener for MemoryBlob {
    fn open_blob(&self) -> Result<Box<dyn Read>> {
        Ok(Box::new(Cursor::new(Arc::clone(&self.bytes))))
    }
}

struct FileBlob {
    path: PathBuf,
}

impl FileBlob {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl BlobOpener for FileBlob {
    fn open_blob(&self) -> Result<Box<dyn Read>> {
        Ok(Box::new(File::open(&self.path)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use containerregistry_image::Digest;
    use std::collections::BTreeMap;

    fn make_descriptor(media_type: MediaType) -> Descriptor {
        Descriptor {
            media_type,
            digest: Digest::sha256(b"test"),
            size: 0,
            urls: vec![],
            annotations: BTreeMap::new(),
            data: None,
            platform: None,
        }
    }

    #[test]
    fn unsupported_media_type_errors() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"").unwrap();
        let layer = TarLayer {
            descriptor: make_descriptor(MediaType::OciManifest),
            opener: Box::new(FileBlob::new(tmp.path().to_path_buf())),
        };
        match layer.open() {
            Err(Error::UnsupportedMediaType(_)) => {}
            Err(e) => panic!("expected UnsupportedMediaType, got {e:?}"),
            Ok(_) => panic!("expected error for manifest media type"),
        }
    }

    #[test]
    fn corrupt_gzip_errors_when_read() {
        // A GzDecoder-wrapped reader should fail once consumed, not at
        // construction — verify the wrapping reaches the caller intact.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"not a gzip stream").unwrap();
        let layer = TarLayer {
            descriptor: make_descriptor(MediaType::OciLayerGzip),
            opener: Box::new(FileBlob::new(tmp.path().to_path_buf())),
        };
        let mut reader = layer.open().unwrap();
        let mut buf = Vec::new();
        assert!(reader.read_to_end(&mut buf).is_err());
    }
}
