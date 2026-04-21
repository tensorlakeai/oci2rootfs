use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use containerregistry_image::ImageConfig;
use containerregistry_layout::Layout;

use crate::error::Result;
use crate::ext4::Ext4Writer;
use crate::ext4_options::Ext4Options;
use crate::oci;
use crate::overlay2::{self, Overlay2Archive};

/// Converts container images to ext4 rootfs images.
pub struct Converter {
    output: PathBuf,
    size: u64,
    ext4_options: Ext4Options,
}

/// A resolved image source ready to be applied to an ext4 writer.
///
/// The source is [`Send`], so callers can resolve it on an async runtime and
/// then move it into a blocking worker thread for [`Converter::convert`].
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

pub(crate) trait SourceImpl: Send {
    fn layer_count(&self) -> usize;
    fn config(&self) -> Option<&ImageConfig>;
    fn apply_to(&self, writer: &mut Ext4Writer) -> Result<()>;
}

impl Converter {
    /// Create a new converter with the given output path.
    pub fn new(output: impl AsRef<Path>) -> Self {
        Self {
            output: output.as_ref().to_path_buf(),
            size: DEFAULT_SIZE,
            ext4_options: Ext4Options::default(),
        }
    }

    /// Set the output ext4 image size in bytes.
    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// Set ext4 image metadata overrides (volume label, UUID).
    ///
    /// Passed to the formatter at creation time. See [`Ext4Options`] for the
    /// set of adjustable fields and their constraints.
    pub fn ext4_options(mut self, opts: Ext4Options) -> Self {
        self.ext4_options = opts;
        self
    }

    /// Convert the provided image source into an ext4 rootfs image.
    ///
    /// If the apply step fails partway through (bad layer, I/O error, out
    /// of space), any partial output file is removed before returning the
    /// error.
    pub fn convert(self, source: impl IntoImageSource) -> Result<()> {
        let started = Instant::now();
        let source = source.into_image_source()?;

        tracing::info!(
            output = %self.output.display(),
            size = self.size,
            layer_count = source.layer_count(),
            "creating ext4 image"
        );

        let mut guard = PartialOutputGuard::new(&self.output);

        let mut writer = Ext4Writer::create(&self.output, self.size, &self.ext4_options)?;
        source.apply_to(&mut writer)?;
        writer.finish()?;

        guard.disarm();

        tracing::info!(
            output = %self.output.display(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "conversion complete"
        );
        Ok(())
    }
}

/// RAII guard that removes a partially-written output file when dropped
/// without being explicitly disarmed. Ensures a failed `convert` doesn't
/// leave a half-baked `.ext4` on disk.
struct PartialOutputGuard<'a> {
    path: &'a Path,
    armed: bool,
}

impl<'a> PartialOutputGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PartialOutputGuard<'_> {
    fn drop(&mut self) {
        if self.armed && self.path.exists() {
            let _ = fs::remove_file(self.path);
        }
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

    /// Returns the OCI image config (entrypoint, cmd, env, working_dir,
    /// user, architecture, os, etc.) when the source carries one.
    ///
    /// `OciLayoutSource` and `RemoteRef::fetch` surface the config. Docker
    /// overlay2 storage keeps the config outside the layer directory
    /// (under `/var/lib/docker/image/overlay2/imagedb/`), which this crate
    /// does not read — `Overlay2Source` therefore returns `None`.
    pub fn config(&self) -> Option<&ImageConfig> {
        self.inner.config()
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
