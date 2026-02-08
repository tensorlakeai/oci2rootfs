# oci2rootfs

Convert OCI container images to ext4 rootfs filesystem images.

## Features

- **Local conversion** — read from an OCI Image Layout directory on disk
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
use oci2rootfs::{Converter, PullConfig};

// Local OCI layout
Converter::new("rootfs.ext4")
    .size(1024 * 1024 * 1024)
    .convert_local("./oci-dir")?;

// Remote registry
let config = PullConfig::default();
Converter::new("rootfs.ext4")
    .convert_remote("nginx:latest", &config)
    .await?;
```

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
│       └── pull.rs      # Remote registry pull
└── oci2rootfs-cli/      # CLI binary
    └── src/
        └── main.rs
```

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
