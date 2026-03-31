use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::ext4::Ext4Writer;
use crate::layer::apply_layer;
use crate::oci;
use crate::pull::{self, PullConfig};

/// Default image size: 512 MiB.
const DEFAULT_SIZE: u64 = 512 * 1024 * 1024;

/// Converts an OCI Image Layout directory to an ext4 rootfs image.
pub struct Converter {
    output: PathBuf,
    size: u64,
}

impl Converter {
    /// Create a new converter with the given output path.
    pub fn new(output: impl AsRef<Path>) -> Self {
        Self {
            output: output.as_ref().to_path_buf(),
            size: DEFAULT_SIZE,
        }
    }

    /// Set the output ext4 image size in bytes.
    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// Convert from a local OCI Image Layout directory.
    pub fn convert_local(self, oci_dir: impl AsRef<Path>) -> Result<()> {
        let image = oci::resolve(oci_dir)?;

        eprintln!("Resolved image with {} layers", image.layers.len());

        let mut writer = Ext4Writer::create(&self.output, self.size)?;

        for (i, layer) in image.layers.iter().enumerate() {
            eprintln!(
                "Applying layer {}/{}: {}",
                i + 1,
                image.layers.len(),
                layer.digest
            );
            let reader = image.open_layer(layer)?;
            apply_layer(reader, &mut writer)?;
        }

        writer.finish()?;
        eprintln!("Created rootfs: {}", self.output.display());
        Ok(())
    }

    /// Convert from a remote registry reference.
    pub async fn convert_remote(self, reference: &str, pull_config: &PullConfig) -> Result<()> {
        let image = pull::pull(reference, pull_config).await?;

        eprintln!("Pulled image with {} layers", image.layer_count());

        let mut writer = Ext4Writer::create(&self.output, self.size)?;

        for (i, desc) in image.layer_descriptors().iter().enumerate() {
            eprintln!(
                "Applying layer {}/{}: {}",
                i + 1,
                image.layer_count(),
                desc.digest
            );
            let reader = image.open_layer(i)?;
            apply_layer(reader, &mut writer)?;
        }

        writer.finish()?;
        eprintln!("Created rootfs: {}", self.output.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_converter_default_size() {
        let c = Converter::new("/tmp/test.ext4");
        assert_eq!(c.size, DEFAULT_SIZE);
        assert_eq!(c.size, 512 * 1024 * 1024);
    }

    #[test]
    fn test_converter_custom_size() {
        let c = Converter::new("/tmp/test.ext4").size(1024 * 1024 * 1024);
        assert_eq!(c.size, 1024 * 1024 * 1024);
    }

    #[test]
    fn test_converter_output_path() {
        let c = Converter::new("/tmp/output.ext4");
        assert_eq!(c.output, PathBuf::from("/tmp/output.ext4"));
    }
}
