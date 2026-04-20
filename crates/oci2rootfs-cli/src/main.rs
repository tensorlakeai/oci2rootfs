use std::path::{Path, PathBuf};

use clap::Parser;
use oci2rootfs::{Converter, Ext4Options, OciLayoutSource, Overlay2Source, Platform, RemoteRef};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "oci2rootfs",
    about = "Convert container images to ext4 rootfs images"
)]
struct Cli {
    /// Source: local OCI layout directory, Docker overlay2 layer directory,
    /// or remote image reference
    /// (e.g., ./oci-dir, /var/lib/docker/overlay2/<id>, nginx:1.21)
    source: String,

    /// Output ext4 image path
    #[arg(short, long)]
    output: PathBuf,

    /// Image size (e.g., "512M", "1G", "128M")
    #[arg(long, default_value = "512M", value_parser = parse_size)]
    size: u64,

    /// Target platform (e.g., "linux/amd64", "linux/arm64")
    #[arg(long, default_value = "linux/amd64")]
    platform: String,

    /// Allow insecure (HTTP) registry connections
    #[arg(long)]
    insecure: bool,

    /// Volume label written to the ext4 superblock (≤16 bytes).
    #[arg(long)]
    label: Option<String>,

    /// Filesystem UUID written to the ext4 superblock. Defaults to a random
    /// v4 UUID assigned by the formatter.
    #[arg(long)]
    uuid: Option<Uuid>,
}

fn parse_size(s: &str) -> std::result::Result<u64, String> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('G').or_else(|| s.strip_suffix('g')) {
        num.parse::<u64>()
            .map(|n| n * 1024 * 1024 * 1024)
            .map_err(|e| e.to_string())
    } else if let Some(num) = s.strip_suffix('M').or_else(|| s.strip_suffix('m')) {
        num.parse::<u64>()
            .map(|n| n * 1024 * 1024)
            .map_err(|e| e.to_string())
    } else if let Some(num) = s.strip_suffix('K').or_else(|| s.strip_suffix('k')) {
        num.parse::<u64>()
            .map(|n| n * 1024)
            .map_err(|e| e.to_string())
    } else {
        s.parse::<u64>().map_err(|e| e.to_string())
    }
}

fn parse_platform(s: &str) -> std::result::Result<Platform, String> {
    let parts: Vec<&str> = s.split('/').collect();
    match parts.as_slice() {
        [os, arch] => Ok(Platform::new(*os, *arch)),
        _ => Err(format!("invalid platform format: {s}, expected os/arch")),
    }
}

fn build_ext4_options(cli: &Cli) -> Ext4Options {
    let mut opts = Ext4Options::new();
    if let Some(label) = &cli.label {
        opts = opts.label(label.clone());
    }
    if let Some(uuid) = cli.uuid {
        opts = opts.uuid(uuid);
    }
    opts
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let platform = parse_platform(&cli.platform).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let converter = Converter::new(&cli.output)
        .size(cli.size)
        .ext4_options(build_ext4_options(&cli));

    let result = if Path::new(&cli.source).exists() {
        let path = Path::new(&cli.source);
        if Overlay2Source::matches(path) {
            tracing::info!(path = %cli.source, "source is Docker overlay2");
            Overlay2Source::open(path).and_then(|s| converter.convert(s))
        } else {
            tracing::info!(path = %cli.source, "source is OCI image layout");
            OciLayoutSource::open(path)
                .map(|s| s.platform(platform))
                .and_then(|s| converter.convert(s))
        }
    } else {
        tracing::info!(reference = %cli.source, "source is remote registry");
        RemoteRef::new(&cli.source)
            .platform(platform)
            .insecure(cli.insecure)
            .fetch()
            .await
            .and_then(|source| converter.convert(source))
    };

    if let Err(e) = result {
        tracing::error!(error = %e, "conversion failed");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_supports_suffixes() {
        assert_eq!(parse_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("512m").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("128K").unwrap(), 128 * 1024);
        assert_eq!(parse_size("1024").unwrap(), 1024);
    }

    #[test]
    fn parse_size_rejects_garbage() {
        assert!(parse_size("abc").is_err());
    }

    #[test]
    fn parse_platform_round_trip() {
        assert_eq!(
            parse_platform("linux/amd64").unwrap(),
            Platform::new("linux", "amd64")
        );
    }

    #[test]
    fn parse_platform_rejects_bad_shape() {
        assert!(parse_platform("invalid").is_err());
        assert!(parse_platform("a/b/c").is_err());
    }
}
