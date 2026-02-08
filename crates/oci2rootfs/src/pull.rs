use std::io::{Cursor, Read};

use containerregistry_auth::AuthResolver;
use containerregistry_image::{Descriptor, ImageConfig, ImageIndex, MediaType};
use containerregistry_registry::{Client, ClientConfig, ManifestOrIndex, Reference};
use flate2::read::GzDecoder;

use crate::error::{Error, Result};

/// Configuration for pulling images from registries.
pub struct PullConfig {
    /// Whether to allow insecure (HTTP) connections.
    pub insecure: bool,
    /// Target CPU architecture (default: "amd64").
    pub arch: String,
    /// Target operating system (default: "linux").
    pub os: String,
}

impl Default for PullConfig {
    fn default() -> Self {
        Self {
            insecure: false,
            arch: "amd64".into(),
            os: "linux".into(),
        }
    }
}

/// A pulled image with config and in-memory layer blobs.
pub struct PulledImage {
    pub config: ImageConfig,
    layers: Vec<(Descriptor, Vec<u8>)>,
}

impl PulledImage {
    /// Returns the number of layers.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Returns the layer descriptors.
    pub fn layer_descriptors(&self) -> Vec<&Descriptor> {
        self.layers.iter().map(|(d, _)| d).collect()
    }

    /// Open a layer blob for reading, decompressing based on media type.
    pub fn open_layer(&self, index: usize) -> Result<Box<dyn Read>> {
        let (descriptor, blob) = &self.layers[index];
        let cursor = Cursor::new(blob.clone());

        match descriptor.media_type {
            MediaType::OciLayerGzip | MediaType::DockerLayerGzip => {
                Ok(Box::new(GzDecoder::new(cursor)))
            }
            MediaType::OciLayerZstd => Ok(Box::new(zstd::Decoder::new(cursor)?)),
            MediaType::OciLayer => Ok(Box::new(cursor)),
            ref other => Err(Error::UnsupportedMediaType(other.as_str().to_string())),
        }
    }
}

/// Pull an image from a remote registry.
///
/// Resolves credentials from Docker config, fetches the manifest (handling
/// image indexes for multi-arch), downloads all layer blobs and the config.
pub async fn pull(reference_str: &str, config: &PullConfig) -> Result<PulledImage> {
    let reference: Reference = reference_str
        .parse()
        .map_err(|e: containerregistry_registry::Error| Error::InvalidReference(e.to_string()))?;

    eprintln!(
        "Pulling {}/{} from {}",
        reference.repository(),
        reference.tag().unwrap_or("latest"),
        reference.registry()
    );

    // Resolve credentials
    let resolver = AuthResolver::new();
    let credential = resolver.resolve_or_anonymous(reference.registry());

    // Create client
    let client_config = ClientConfig::new()
        .with_https(!config.insecure)
        .with_insecure(config.insecure);
    let client = Client::with_credential(client_config, credential)?;

    // Fetch manifest or index
    let (manifest_or_index, _digest) = client.get_manifest(&reference).await?;

    // Resolve to a platform-specific manifest
    let manifest = match manifest_or_index {
        ManifestOrIndex::Manifest(m) => *m,
        ManifestOrIndex::Index(index) => {
            resolve_platform_manifest(&client, &reference, &index, config).await?
        }
    };

    eprintln!("Downloading {} layers", manifest.layers().len());

    // Download config
    let config_data = client.get_blob(&reference, &manifest.config().digest).await?;
    let image_config = ImageConfig::from_bytes(&config_data)?;

    // Download layers
    let mut layers = Vec::new();
    for (i, layer_desc) in manifest.layers().iter().enumerate() {
        eprintln!(
            "Downloading layer {}/{}: {} ({} bytes)",
            i + 1,
            manifest.layers().len(),
            layer_desc.digest,
            layer_desc.size
        );
        let blob = client.get_blob(&reference, &layer_desc.digest).await?;
        layers.push((layer_desc.clone(), blob));
    }

    Ok(PulledImage {
        config: image_config,
        layers,
    })
}

#[cfg(test)]
impl PulledImage {
    fn new_for_test(layers: Vec<(Descriptor, Vec<u8>)>) -> Self {
        let config_json = br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#;
        Self {
            config: ImageConfig::from_bytes(config_json).unwrap(),
            layers,
        }
    }
}

/// Resolve a platform-specific manifest from an image index.
async fn resolve_platform_manifest(
    client: &Client,
    reference: &Reference,
    index: &ImageIndex,
    config: &PullConfig,
) -> Result<containerregistry_image::Manifest> {
    eprintln!("Resolving platform {}/{}", config.os, config.arch);

    let desc = index
        .find_platform(&config.arch, &config.os, None)
        .ok_or_else(|| Error::NoManifest(format!("{}/{}", config.os, config.arch)))?;

    // Fetch the platform-specific manifest by digest
    let manifest_ref = Reference::with_digest(
        reference.registry().to_string(),
        reference.repository().to_string(),
        desc.digest.clone(),
    );

    let (manifest_or_index, _) = client.get_manifest(&manifest_ref).await?;

    match manifest_or_index {
        ManifestOrIndex::Manifest(m) => Ok(*m),
        ManifestOrIndex::Index(_) => Err(Error::UnsupportedMediaType(
            "nested image index not supported".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use containerregistry_image::Digest;
    use std::collections::BTreeMap;
    use std::io::Read;

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
    fn test_pull_config_default() {
        let config = PullConfig::default();
        assert!(!config.insecure);
        assert_eq!(config.arch, "amd64");
        assert_eq!(config.os, "linux");
    }

    #[test]
    fn test_pulled_image_layer_count() {
        let image = PulledImage::new_for_test(vec![
            (make_descriptor(MediaType::OciLayer), vec![1, 2, 3]),
        ]);
        assert_eq!(image.layer_count(), 1);
    }

    #[test]
    fn test_pulled_image_layer_descriptors() {
        let desc = make_descriptor(MediaType::OciLayer);
        let image = PulledImage::new_for_test(vec![(desc.clone(), vec![])]);
        let descs = image.layer_descriptors();
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].digest, desc.digest);
    }

    #[test]
    fn test_open_layer_uncompressed() {
        let data = b"hello world";
        let image = PulledImage::new_for_test(vec![
            (make_descriptor(MediaType::OciLayer), data.to_vec()),
        ]);
        let mut reader = image.open_layer(0).unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "hello world");
    }

    #[test]
    fn test_open_layer_gzip() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(b"compressed data").unwrap();
        let compressed = encoder.finish().unwrap();

        let image = PulledImage::new_for_test(vec![
            (make_descriptor(MediaType::OciLayerGzip), compressed),
        ]);
        let mut reader = image.open_layer(0).unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "compressed data");
    }

    #[test]
    fn test_open_layer_unsupported_media_type() {
        let image = PulledImage::new_for_test(vec![
            (make_descriptor(MediaType::OciManifest), vec![]),
        ]);
        match image.open_layer(0) {
            Err(Error::UnsupportedMediaType(_)) => {} // expected
            Err(e) => panic!("unexpected error: {e}"),
            Ok(_) => panic!("expected error for unsupported media type"),
        }
    }
}
