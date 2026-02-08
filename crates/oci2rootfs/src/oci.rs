use std::fs;
use std::io::Read;
use std::path::Path;

use containerregistry_image::{Descriptor, ImageConfig, ImageIndex, MediaType};
use containerregistry_layout::Layout;
use flate2::read::GzDecoder;

use crate::error::{Error, Result};

/// A fully resolved image with manifest, config, and layer descriptors.
pub struct ResolvedImage {
    pub config: ImageConfig,
    pub layers: Vec<Descriptor>,
    layout: Layout,
}

/// Open an OCI Image Layout and resolve the manifest for the current platform.
///
/// Reads index.json, finds the appropriate manifest (preferring linux/amd64),
/// then reads the manifest and config.
pub fn resolve(path: impl AsRef<Path>) -> Result<ResolvedImage> {
    let layout = Layout::open(path)?;
    let index = layout.index()?;

    // Find the manifest descriptor for our target platform
    let manifest_desc = find_manifest_descriptor(&layout, &index)?;

    // Read the manifest
    let manifest = layout.read_manifest(&manifest_desc.digest)?;

    // Read the config
    let config = layout.read_config(&manifest.config().digest)?;

    // Collect layer descriptors
    let layers = manifest.layers().to_vec();

    Ok(ResolvedImage {
        config,
        layers,
        layout,
    })
}

impl ResolvedImage {
    /// Open a layer blob for reading, decompressing based on media type.
    pub fn open_layer(&self, layer: &Descriptor) -> Result<Box<dyn Read>> {
        let blob_path = self.layout.blob_path(&layer.digest);
        let file = fs::File::open(&blob_path)?;

        match layer.media_type {
            MediaType::OciLayerGzip | MediaType::DockerLayerGzip => {
                Ok(Box::new(GzDecoder::new(file)))
            }
            MediaType::OciLayerZstd => Ok(Box::new(zstd::Decoder::new(file)?)),
            MediaType::OciLayer => Ok(Box::new(file)),
            ref other => Err(Error::UnsupportedMediaType(other.as_str().to_string())),
        }
    }
}

/// Find the best manifest descriptor from an image index.
fn find_manifest_descriptor(layout: &Layout, index: &ImageIndex) -> Result<Descriptor> {
    let descriptors = index.manifests();

    // If only one descriptor and it's a manifest (not a nested index), use it directly
    if descriptors.len() == 1 && !descriptors[0].media_type.is_index() {
        return Ok(descriptors[0].clone());
    }

    // Try platform-aware selection: linux/amd64
    if let Some(desc) = index.find_platform("amd64", "linux", None) {
        if desc.media_type.is_index() {
            // Nested index — resolve
            let nested_data = layout.read_blob(&desc.digest)?;
            let nested = ImageIndex::from_bytes(&nested_data)?;
            return find_manifest_descriptor(layout, &nested);
        }
        return Ok(desc.clone());
    }

    // For entries pointing to another index, resolve recursively
    for desc in descriptors {
        if desc.media_type.is_index() {
            let nested_data = layout.read_blob(&desc.digest)?;
            let nested = ImageIndex::from_bytes(&nested_data)?;
            if let Ok(resolved) = find_manifest_descriptor(layout, &nested) {
                return Ok(resolved);
            }
        }
    }

    // Fallback: first manifest-type descriptor
    for desc in descriptors {
        if desc.media_type.is_manifest() {
            return Ok(desc.clone());
        }
    }

    Err(Error::NoManifest("linux/amd64".into()))
}
