use crate::convert::{ImageSource, Platform};
use crate::error::{Error, Result};
use crate::tar_source::TarImageSource;

use containerregistry_auth::AuthResolver;
use containerregistry_image::ImageIndex;
use containerregistry_registry::{Client, ClientConfig, ManifestOrIndex, Reference};

/// Builder for a remote registry reference that will be fetched into memory.
pub struct RemoteRef {
    reference: String,
    platform: Platform,
    insecure: bool,
}

impl RemoteRef {
    /// Create a remote reference builder from an image reference string.
    pub fn new(reference: impl Into<String>) -> Self {
        Self {
            reference: reference.into(),
            platform: Platform::default(),
            insecure: false,
        }
    }

    /// Override the platform used when resolving the remote manifest.
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }

    /// Allow insecure (HTTP) registry connections for this reference.
    pub fn insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    /// Fetch the remote image and materialize it into an in-memory [`ImageSource`].
    pub async fn fetch(self) -> Result<ImageSource> {
        Ok(ImageSource::new(
            pull(&self.reference, self.insecure, &self.platform).await?,
        ))
    }
}

/// Pull an image from a remote registry and materialize its layers in memory.
async fn pull(reference_str: &str, insecure: bool, platform: &Platform) -> Result<TarImageSource> {
    let reference: Reference = reference_str
        .parse()
        .map_err(|e: containerregistry_registry::Error| Error::InvalidReference(e.to_string()))?;

    eprintln!(
        "Pulling {}/{} from {}",
        reference.repository(),
        reference.tag().unwrap_or("latest"),
        reference.registry()
    );

    let resolver = AuthResolver::new();
    let credential = resolver.resolve_or_anonymous(reference.registry());

    let client_config = ClientConfig::new()
        .with_https(!insecure)
        .with_insecure(insecure);
    let client = Client::with_credential(client_config, credential)?;

    let (manifest_or_index, _digest) = client.get_manifest(&reference).await?;
    let manifest = match manifest_or_index {
        ManifestOrIndex::Manifest(manifest) => *manifest,
        ManifestOrIndex::Index(index) => {
            resolve_platform_manifest(&client, &reference, &index, platform).await?
        }
    };

    let config_data = client
        .get_blob(&reference, &manifest.config().digest)
        .await?;
    let config = containerregistry_image::ImageConfig::from_bytes(&config_data)?;

    let mut layers = Vec::with_capacity(manifest.layers().len());
    for (index, layer_desc) in manifest.layers().iter().enumerate() {
        eprintln!(
            "Downloading layer {}/{}: {} ({} bytes)",
            index + 1,
            manifest.layers().len(),
            layer_desc.digest,
            layer_desc.size
        );
        let blob = client.get_blob(&reference, &layer_desc.digest).await?;
        layers.push((layer_desc.clone(), blob));
    }

    eprintln!(
        "Pulled remote image for {}/{} with {} layers",
        platform.os(),
        platform.arch(),
        layers.len()
    );

    Ok(TarImageSource::from_memory(config, layers, "remote"))
}

/// Resolve a platform-specific manifest from a multi-platform image index.
async fn resolve_platform_manifest(
    client: &Client,
    reference: &Reference,
    index: &ImageIndex,
    platform: &Platform,
) -> Result<containerregistry_image::Manifest> {
    eprintln!("Resolving platform {}/{}", platform.os(), platform.arch());

    let desc = index
        .find_platform(platform.arch(), platform.os(), None)
        .ok_or_else(|| Error::NoManifest(format!("{}/{}", platform.os(), platform.arch())))?;

    let manifest_ref = Reference::with_digest(
        reference.registry().to_string(),
        reference.repository().to_string(),
        desc.digest.clone(),
    );

    let (manifest_or_index, _) = client.get_manifest(&manifest_ref).await?;

    match manifest_or_index {
        ManifestOrIndex::Manifest(manifest) => Ok(*manifest),
        ManifestOrIndex::Index(_) => Err(Error::UnsupportedMediaType(
            "nested image index not supported".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use containerregistry_image::{Descriptor, Digest, MediaType};
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
    fn remote_ref_defaults() {
        let remote = RemoteRef::new("alpine:latest");
        assert_eq!(remote.reference, "alpine:latest");
        assert_eq!(remote.platform, Platform::default());
        assert!(!remote.insecure);
    }

    #[test]
    fn tar_source_from_memory_reports_layer_count() {
        let image = TarImageSource::from_memory(
            containerregistry_image::ImageConfig::from_bytes(
                br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#,
            )
            .unwrap(),
            vec![(make_descriptor(MediaType::OciLayer), vec![1, 2, 3])],
            "remote",
        );

        assert_eq!(crate::convert::SourceImpl::layer_count(&image), 1);
    }

    #[test]
    fn tar_source_open_layer_gzip() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(b"compressed data").unwrap();
        let compressed = encoder.finish().unwrap();

        let image = TarImageSource::from_memory(
            containerregistry_image::ImageConfig::from_bytes(
                br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#,
            )
            .unwrap(),
            vec![(make_descriptor(MediaType::OciLayerGzip), compressed)],
            "remote",
        );

        let mut reader = image.open_layer(0).unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "compressed data");
    }
}
