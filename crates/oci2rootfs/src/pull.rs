use crate::convert::{ImageSource, Platform};
use crate::error::{Error, Result};
use crate::tar_source::TarImageSource;

use containerregistry_auth::AuthResolver;
use containerregistry_image::{Digest, ImageIndex};
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
        let (source, manifest_digest) =
            pull(&self.reference, self.insecure, &self.platform).await?;
        Ok(ImageSource::with_manifest_digest(
            source,
            Some(manifest_digest),
        ))
    }
}

/// Pull an image from a remote registry and materialize its layers in memory.
async fn pull(
    reference_str: &str,
    insecure: bool,
    platform: &Platform,
) -> Result<(TarImageSource, Digest)> {
    let reference: Reference = reference_str
        .parse()
        .map_err(|e: containerregistry_registry::Error| Error::InvalidReference(e.to_string()))?;

    tracing::info!(
        registry = reference.registry(),
        repository = reference.repository(),
        tag = reference.tag().unwrap_or("latest"),
        "pulling image from registry"
    );

    let resolver = AuthResolver::new();
    let credential = resolver.resolve_or_anonymous(reference.registry());

    let client_config = ClientConfig::new()
        .with_https(!insecure)
        .with_insecure(insecure);
    let client = Client::with_credential(client_config, credential)?;

    let (manifest_or_index, digest) = client.get_manifest(&reference).await?;
    let (manifest, manifest_digest) = match manifest_or_index {
        ManifestOrIndex::Manifest(manifest) => (*manifest, digest),
        ManifestOrIndex::Index(index) => {
            resolve_platform_manifest(&client, &reference, &index, platform).await?
        }
    };

    tracing::info!(
        os = platform.os(),
        arch = platform.arch(),
        manifest_digest = %manifest_digest,
        layer_count = manifest.layers().len(),
        "resolved remote manifest"
    );

    let config_data = client
        .get_blob(&reference, &manifest.config().digest)
        .await?;
    let config = containerregistry_image::ImageConfig::from_bytes(&config_data)?;

    let layer_count = manifest.layers().len();
    let mut layers = Vec::with_capacity(layer_count);
    for (index, layer_desc) in manifest.layers().iter().enumerate() {
        let blob = client.get_blob(&reference, &layer_desc.digest).await?;
        tracing::info!(
            layer_index = index + 1,
            layer_count,
            digest = %layer_desc.digest,
            bytes = layer_desc.size,
            "downloaded layer blob"
        );
        layers.push((layer_desc.clone(), blob));
    }

    Ok((TarImageSource::from_memory(config, layers), manifest_digest))
}

/// Resolve a platform-specific manifest from a multi-platform image index.
async fn resolve_platform_manifest(
    client: &Client,
    reference: &Reference,
    index: &ImageIndex,
    platform: &Platform,
) -> Result<(containerregistry_image::Manifest, Digest)> {
    let desc = index
        .find_platform(platform.arch(), platform.os(), None)
        .ok_or_else(|| Error::NoManifest(format!("{}/{}", platform.os(), platform.arch())))?;

    let manifest_ref = Reference::with_digest(
        reference.registry().to_string(),
        reference.repository().to_string(),
        desc.digest.clone(),
    );

    let (manifest_or_index, manifest_digest) = client.get_manifest(&manifest_ref).await?;

    match manifest_or_index {
        ManifestOrIndex::Manifest(manifest) => Ok((*manifest, manifest_digest)),
        ManifestOrIndex::Index(_) => Err(Error::UnsupportedMediaType(
            "nested image index not supported".into(),
        )),
    }
}
