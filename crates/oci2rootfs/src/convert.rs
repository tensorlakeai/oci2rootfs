use std::path::{Path, PathBuf};

use containerregistry_layout::Layout;

use crate::error::Result;
use crate::ext4::Ext4Writer;
use crate::oci;
use crate::overlay2::{self, Overlay2Archive};

/// Converts container images to ext4 rootfs images.
pub struct Converter {
    output: PathBuf,
    size: u64,
}

/// A resolved image source ready to be applied to an ext4 writer.
pub struct ImageSource {
    inner: Box<dyn SourceImpl>,
}

/// Converts a builder or already-resolved source into an [`ImageSource`].
pub trait IntoImageSource {
    /// Resolve this value into the concrete image source used by [`Converter`].
    fn into_image_source(self) -> Result<ImageSource>;
}

/// Target platform used when resolving manifest lists and image indexes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Platform {
    os: String,
    arch: String,
}

/// Builder for an OCI image layout source on disk.
pub struct OciLayoutSource {
    layout: Layout,
    platform: Platform,
}

/// Builder for a Docker overlay2 layer chain on disk.
pub struct Overlay2Source {
    archive: Overlay2Archive,
}

/// Default image size: 512 MiB.
const DEFAULT_SIZE: u64 = 512 * 1024 * 1024;

pub(crate) trait SourceImpl {
    fn layer_count(&self) -> usize;
    fn apply_to(&self, writer: &mut Ext4Writer) -> Result<()>;
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

    /// Convert the provided image source into an ext4 rootfs image.
    pub fn convert(self, source: impl IntoImageSource) -> Result<()> {
        let source = source.into_image_source()?;
        let mut writer = Ext4Writer::create(&self.output, self.size)?;
        source.apply_to(&mut writer)?;
        writer.finish()?;
        Ok(())
    }
}

impl ImageSource {
    pub(crate) fn new(inner: impl SourceImpl + 'static) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    /// Returns the number of layers that will be applied.
    pub fn layer_count(&self) -> usize {
        self.inner.layer_count()
    }

    fn apply_to(&self, writer: &mut Ext4Writer) -> Result<()> {
        self.inner.apply_to(writer)
    }
}

impl IntoImageSource for ImageSource {
    fn into_image_source(self) -> Result<ImageSource> {
        Ok(self)
    }
}

impl Default for Platform {
    fn default() -> Self {
        Self::new("linux", "amd64")
    }
}

impl Platform {
    /// Create a platform selector from an operating system and architecture.
    pub fn new(os: impl Into<String>, arch: impl Into<String>) -> Self {
        Self {
            os: os.into(),
            arch: arch.into(),
        }
    }

    /// Returns the operating system component.
    pub fn os(&self) -> &str {
        &self.os
    }

    /// Returns the architecture component.
    pub fn arch(&self) -> &str {
        &self.arch
    }
}

impl OciLayoutSource {
    /// Open an OCI image layout directory on disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let layout = Layout::open(path)?;
        Ok(Self {
            layout,
            platform: Platform::default(),
        })
    }

    /// Override the platform used when resolving the layout's manifest.
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }
}

impl IntoImageSource for OciLayoutSource {
    fn into_image_source(self) -> Result<ImageSource> {
        Ok(ImageSource::new(oci::resolve(self.layout, &self.platform)?))
    }
}

impl Overlay2Source {
    /// Open and resolve a Docker overlay2 chain-id directory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            archive: overlay2::resolve(path.as_ref())?,
        })
    }

    /// Returns whether the path looks like a Docker overlay2 layer directory.
    pub fn matches(path: &Path) -> bool {
        overlay2::is_overlay2(path)
    }
}

impl IntoImageSource for Overlay2Source {
    fn into_image_source(self) -> Result<ImageSource> {
        Ok(ImageSource::new(self.archive))
    }
}

/// Auto-detect a local image source from an on-disk path.
///
/// Docker overlay2 layer directories are detected by their `diff/` and `link`
/// markers. All other paths are treated as OCI image layouts and resolved for
/// the default platform (`linux/amd64`). Callers that need a different
/// platform for OCI layouts should use [`OciLayoutSource::open`] directly.
pub fn autodetect(path: impl AsRef<Path>) -> Result<ImageSource> {
    let path = path.as_ref();

    if Overlay2Source::matches(path) {
        Overlay2Source::open(path)?.into_image_source()
    } else {
        OciLayoutSource::open(path)?.into_image_source()
    }
}
