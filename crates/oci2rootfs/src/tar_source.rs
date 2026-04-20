use std::fs::File;
#[cfg(any(feature = "remote", test))]
use std::io::Cursor;
use std::io::Read;
use std::path::PathBuf;
#[cfg(any(feature = "remote", test))]
use std::sync::Arc;

use containerregistry_image::{Descriptor, ImageConfig, MediaType};
use flate2::read::GzDecoder;

use crate::convert::SourceImpl;
use crate::error::{Error, Result};
use crate::ext4::Ext4Writer;
use crate::layer::apply_layer;

/// A tar-layer-backed image source that can read blobs from memory or disk.
pub(crate) struct TarImageSource {
    #[allow(dead_code)]
    config: ImageConfig,
    layers: Vec<TarLayer>,
    source_label: &'static str,
}

impl TarImageSource {
    /// Build a source whose layer blobs already live in memory.
    #[cfg(any(feature = "remote", test))]
    pub(crate) fn from_memory(
        config: ImageConfig,
        layers: Vec<(Descriptor, Vec<u8>)>,
        source_label: &'static str,
    ) -> Self {
        let layers = layers
            .into_iter()
            .map(|(descriptor, blob)| TarLayer {
                descriptor,
                opener: Box::new(MemoryBlob::new(blob)),
            })
            .collect();

        Self {
            config,
            layers,
            source_label,
        }
    }

    /// Build a source whose layer blobs live on disk.
    pub(crate) fn from_files(
        config: ImageConfig,
        layers: Vec<(Descriptor, PathBuf)>,
        source_label: &'static str,
    ) -> Self {
        let layers = layers
            .into_iter()
            .map(|(descriptor, path)| TarLayer {
                descriptor,
                opener: Box::new(FileBlob::new(path)),
            })
            .collect();

        Self {
            config,
            layers,
            source_label,
        }
    }

    #[cfg(test)]
    pub(crate) fn config(&self) -> &ImageConfig {
        &self.config
    }

    #[cfg(test)]
    pub(crate) fn open_layer(&self, index: usize) -> Result<Box<dyn Read>> {
        self.layers[index].open()
    }
}

impl SourceImpl for TarImageSource {
    fn layer_count(&self) -> usize {
        self.layers.len()
    }

    fn apply_to(&self, writer: &mut Ext4Writer) -> Result<()> {
        for (index, layer) in self.layers.iter().enumerate() {
            eprintln!(
                "Applying {} layer {}/{}: {}",
                self.source_label,
                index + 1,
                self.layers.len(),
                layer.descriptor.digest
            );
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

#[cfg(any(feature = "remote", test))]
struct MemoryBlob {
    bytes: Arc<[u8]>,
}

#[cfg(any(feature = "remote", test))]
impl MemoryBlob {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::from(bytes),
        }
    }
}

#[cfg(any(feature = "remote", test))]
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
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::collections::BTreeMap;
    use std::io::Write;
    use tempfile::TempDir;

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
    fn open_memory_blob_layer() {
        let layer = TarLayer {
            descriptor: make_descriptor(MediaType::OciLayer),
            opener: Box::new(MemoryBlob::new(b"hello world".to_vec())),
        };

        let mut reader = layer.open().unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "hello world");
    }

    #[test]
    fn open_file_blob_layer() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("layer.tar.gz");

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(b"compressed data").unwrap();
        let compressed = encoder.finish().unwrap();
        std::fs::write(&path, compressed).unwrap();

        let layer = TarLayer {
            descriptor: make_descriptor(MediaType::OciLayerGzip),
            opener: Box::new(FileBlob::new(path)),
        };

        let mut reader = layer.open().unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "compressed data");
    }
}
