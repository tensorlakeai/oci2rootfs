//! Convert OCI container images to ext4 rootfs filesystem images.
//!
//! The crate centers on one method — [`Converter::convert`] — which consumes
//! any value implementing [`IntoImageSource`] and writes an ext4 image to
//! disk. Three source types are provided:
//!
//! - [`OciLayoutSource`] — a local OCI Image Layout directory (the output of
//!   `skopeo copy`, `docker save` + extract, `buildah push --format=oci`,
//!   etc.).
//! - [`Overlay2Source`] — a Docker overlay2 chain-id directory, typically
//!   under `/var/lib/docker/overlay2/<id>`.
//! - `RemoteRef` — a remote registry reference pulled via HTTPS. Available
//!   only with the `remote` feature (enabled by default).
//!
//! [`autodetect`] picks between `OciLayoutSource` and `Overlay2Source` for a
//! local path based on the directory layout.
//!
//! # Examples
//!
//! Local OCI image layout:
//!
//! ```no_run
//! use oci2rootfs::{Converter, OciLayoutSource, Platform};
//!
//! Converter::new("rootfs.ext4")
//!     .size(1 << 30)
//!     .convert(
//!         OciLayoutSource::open("./layout")?
//!             .platform(Platform::new("linux", "arm64")),
//!     )?;
//! # Ok::<_, oci2rootfs::Error>(())
//! ```
//!
//! Docker overlay2 directory:
//!
//! ```no_run
//! use oci2rootfs::{Converter, Overlay2Source};
//!
//! Converter::new("rootfs.ext4")
//!     .convert(Overlay2Source::open("/var/lib/docker/overlay2/abc123")?)?;
//! # Ok::<_, oci2rootfs::Error>(())
//! ```
//!
//! Remote registry (requires the `remote` feature):
//!
//! ```no_run
//! # #[cfg(feature = "remote")]
//! # async fn run() -> oci2rootfs::Result<()> {
//! use oci2rootfs::{Converter, RemoteRef};
//!
//! let source = RemoteRef::new("alpine:3.19").fetch().await?;
//! Converter::new("alpine.ext4").convert(source)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Feature flags
//!
//! - `remote` *(default)* — enables `RemoteRef` and the registry pull path.
//!   Pulls in `containerregistry-registry`, `containerregistry-auth`,
//!   `tokio`, `reqwest`, and a TLS stack. Set `default-features = false` on
//!   the dependency to drop the network stack when only local sources are
//!   needed.
//!
//! # Security
//!
//! - Tar entry paths and hardlink targets are validated via
//!   [`std::path::Path::components`]: parent-dir (`..`) components and
//!   NUL bytes are rejected as [`Error::InvalidTarPath`]. Symlink targets
//!   are stored verbatim (only NUL bytes and non-UTF-8 are rejected):
//!   relative `..` targets like `/usr/sbin/foo -> ../bin/foo` are
//!   legitimate in real images and are resolved by the kernel against the
//!   consumer's mount point, not the host.
//! - Overlay2 `lower` references that canonicalize outside the overlay2
//!   root directory are rejected.
//! - Remote blob fetches verify SHA-256 digests against the manifest
//!   descriptor (delegated to `containerregistry-registry`).
//! - Output is written into an ext4 image file; the library never writes to
//!   the host filesystem outside that file, and never invokes `mount`,
//!   `chroot`, or other privileged operations.
//!
//! # Not supported
//!
//! - Device nodes (`mknod`). Character/block/FIFO tar entries are skipped.
//! - UID/GID translation. Ownership is written verbatim from the source.

#![deny(missing_docs)]

mod convert;
mod error;
mod ext4;
mod ext4_options;
mod layer;
mod oci;
mod overlay2;
mod path;
#[cfg(feature = "remote")]
mod pull;
mod tar_source;

pub use containerregistry_image::{
    ContainerConfig, Digest, Healthcheck, History, ImageConfig, RootFs,
};
pub use convert::{
    Converter, ImageSource, IntoImageSource, OciLayoutSource, Overlay2Source, Platform, autodetect,
};
pub use error::{Error, Result};
pub use ext4_options::Ext4Options;
#[cfg(feature = "remote")]
pub use pull::RemoteRef;
