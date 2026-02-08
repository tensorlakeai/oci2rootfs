use std::path::PathBuf;

use clap::Parser;
use oci2rootfs::{Converter, PullConfig};

#[derive(Parser)]
#[command(name = "oci2rootfs", about = "Convert OCI images to ext4 rootfs images")]
struct Cli {
    /// Source: local OCI layout directory or remote image reference
    /// (e.g., ./oci-dir, nginx:1.21, gcr.io/project/app@sha256:...)
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

fn parse_platform(s: &str) -> std::result::Result<(String, String), String> {
    let parts: Vec<&str> = s.split('/').collect();
    match parts.as_slice() {
        [os, arch] => Ok((os.to_string(), arch.to_string())),
        _ => Err(format!("invalid platform format: {s}, expected os/arch")),
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let converter = Converter::new(&cli.output).size(cli.size);

    let result = if std::path::Path::new(&cli.source).exists() {
        // Local OCI layout directory
        eprintln!("Source: local OCI layout at {}", cli.source);
        converter.convert_local(&cli.source)
    } else {
        // Remote registry reference
        eprintln!("Source: remote registry {}", cli.source);

        let (os, arch) = parse_platform(&cli.platform).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

        let pull_config = PullConfig {
            insecure: cli.insecure,
            arch,
            os,
        };

        converter.convert_remote(&cli.source, &pull_config).await
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_megabytes() {
        assert_eq!(parse_size("512M").unwrap(), 512 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_gigabytes() {
        assert_eq!(parse_size("1G").unwrap(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_kilobytes() {
        assert_eq!(parse_size("128K").unwrap(), 128 * 1024);
    }

    #[test]
    fn test_parse_size_lowercase() {
        assert_eq!(parse_size("512m").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("1g").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("128k").unwrap(), 128 * 1024);
    }

    #[test]
    fn test_parse_size_raw_bytes() {
        assert_eq!(parse_size("1024").unwrap(), 1024);
    }

    #[test]
    fn test_parse_size_invalid() {
        assert!(parse_size("abc").is_err());
    }

    #[test]
    fn test_parse_platform_valid() {
        assert_eq!(
            parse_platform("linux/amd64").unwrap(),
            ("linux".to_string(), "amd64".to_string())
        );
    }

    #[test]
    fn test_parse_platform_arm() {
        assert_eq!(
            parse_platform("linux/arm64").unwrap(),
            ("linux".to_string(), "arm64".to_string())
        );
    }

    #[test]
    fn test_parse_platform_invalid() {
        assert!(parse_platform("invalid").is_err());
    }

    #[test]
    fn test_parse_platform_too_many() {
        assert!(parse_platform("a/b/c").is_err());
    }
}
