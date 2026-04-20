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
| `--label` | (none) | Volume label written to the superblock (≤16 bytes) |
| `--uuid` | random | Filesystem UUID written to the superblock |

Set `RUST_LOG=info` (or `debug`, `trace`) to see per-layer progress and
timing on stderr — the CLI installs a `tracing-subscriber` fmt layer
wired to `RUST_LOG` via env-filter.

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
Converter::new("rootfs.ext4").convert(autodetect("./some-path")?)?;

// Remote registry (default feature: `remote`)
# #[cfg(feature = "remote")]
# async fn example() -> oci2rootfs::Result<()> {
use oci2rootfs::RemoteRef;

let source = RemoteRef::new("nginx:latest")
    .platform(Platform::default())
    .fetch()
    .await?;

// Image config (entrypoint/cmd/env/...) is available on the source.
if let Some(cfg) = source.config() {
    if let Some(container) = &cfg.config {
        println!("entrypoint: {:?}", container.entrypoint);
    }
}

Converter::new("rootfs.ext4").convert(source)?;
# Ok(())
# }
```

### ext4 image properties

| Property         | Value                                  | Configurable |
|------------------|----------------------------------------|--------------|
| Block size       | 4 KiB                                  | No (arcbox-ext4 fixed) |
| Journal          | none (effectively `mkfs.ext4 -O ^has_journal`) | No |
| Feature flags    | `SPARSE_SUPER2 | EXT_ATTR`             | No |
| Volume label     | empty                                  | `Ext4Options::label` / `--label` |
| UUID             | random v4                              | `Ext4Options::uuid` / `--uuid` |
| `ImageConfig`    | exposed via `ImageSource::config()` for OCI layout and remote pulls; `None` for overlay2 sources (Docker keeps the config outside the layer tree) | — |

### Feature flags

| Feature | Default | Pulls in |
|---------|---------|----------|
| `remote` | yes | `containerregistry-registry`, `containerregistry-auth`, `tokio`, `reqwest`, TLS stack |

Add `default-features = false` on the library dependency to drop the remote
pull stack when you only need OCI layout and Docker overlay2 support:

```toml
[dependencies]
oci2rootfs = { version = "0.1", default-features = false }
```

### Known limitations

- **Remote pull buffers every layer blob in memory** before applying. For
  multi-gigabyte images this requires proportional heap; prefer spooling to an
  OCI layout on disk for large images (e.g. via `skopeo copy docker://<ref>
  oci:./layout:latest`, then `OciLayoutSource::open`).
- **Layer downloads are serial.** Parallel fetches could reduce wall time for
  images with many small layers but would complicate progress reporting.
- **No device-node support.** `mknod`-created entries in tar layers (char/
  block/FIFO) are skipped silently; overlay2 `.wh.<name>` files cover deletion.
- **Block size and inode ratio are not configurable.** `arcbox-ext4` hard-codes
  4 KiB blocks and computes inode counts internally.

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
│       ├── ext4.rs      # ext4 image writer (arcbox-ext4)
│       ├── layer.rs     # Tar layer application with whiteout support
│       ├── oci.rs       # OCI Image Layout resolution
│       ├── ext4_options.rs # Post-format UUID/label superblock rewrite
│       ├── overlay2/    # Docker overlay2 resolution and apply
│       ├── path.rs      # Shared path sanitation + whiteout parsing
│       ├── pull.rs      # Remote registry pull (feature = "remote")
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
