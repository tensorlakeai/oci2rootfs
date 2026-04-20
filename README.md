# oci2rootfs

Convert OCI container images to ext4 rootfs filesystem images.

## Features

- **Local conversion** — read from an OCI Image Layout directory on disk
- **Docker cache paths** — convert directly from Docker `overlay2` chain-id directories
- **Remote pull** — pull images directly from container registries (Docker Hub, GCR, etc.)
- **Multi-arch support** — resolve platform-specific manifests from image indexes
- **Layer handling** — apply tar layers in order with full OCI whiteout support (`.wh.*` deletes, `.wh..wh..opq` opaque)
- **Compression** — gzip, zstd, and uncompressed layers
- **Auth** — automatic credential resolution from Docker config

## Usage

```bash
# From a local OCI layout directory
oci2rootfs ./oci-dir --output rootfs.ext4

# From a remote registry
oci2rootfs nginx:1.21 --output rootfs.ext4

# Custom size and platform
oci2rootfs ubuntu:22.04 --output rootfs.ext4 --size 1G --platform linux/arm64

# Insecure (HTTP) registry
oci2rootfs localhost:5000/myapp:latest --output rootfs.ext4 --insecure
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--output`, `-o` | (required) | Output ext4 image path |
| `--size` | `512M` | Image size (`128M`, `1G`, etc.) |
| `--platform` | `linux/amd64` | Target platform |
| `--insecure` | `false` | Allow HTTP registry connections |

## Library

```rust
use oci2rootfs::{autodetect, Converter, OciLayoutSource, Platform};

// Local OCI layout
Converter::new("rootfs.ext4")
    .size(1024 * 1024 * 1024)
    .convert(
        OciLayoutSource::open("./oci-dir")?
            .platform(Platform::new("linux", "amd64")),
    )?;

// Local auto-detect (OCI layout or overlay2 path)
Converter::new("rootfs.ext4")
    .convert(autodetect("./some-path", Platform::default())?)?;

// Remote registry (default feature: `remote`)
# #[cfg(feature = "remote")]
# async fn example() -> oci2rootfs::Result<()> {
use oci2rootfs::RemoteRef;

let source = RemoteRef::new("nginx:latest")
    .platform(Platform::default())
    .fetch()
    .await?;

Converter::new("rootfs.ext4").convert(source)?;
# Ok(())
# }
```

Use `default-features = false` on the library dependency when you only need
local OCI layout and overlay2 support and want to drop the remote pull stack.

## Build

```bash
cargo build --release
```

## Project Structure

```
crates/
├── oci2rootfs/          # Library
│   └── src/
│       ├── convert.rs   # High-level Converter API
│       ├── error.rs     # Error types
│       ├── ext4.rs      # ext4 image writer (lwext4)
│       ├── layer.rs     # Tar layer application with whiteout support
│       ├── oci.rs       # OCI Image Layout resolution
│       ├── overlay2/    # Docker overlay2 resolution and apply
│       ├── pull.rs      # Remote registry pull
│       └── tar_source.rs # Shared tar-layer source plumbing
└── oci2rootfs-cli/      # CLI binary
    └── src/
        └── main.rs
```

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
