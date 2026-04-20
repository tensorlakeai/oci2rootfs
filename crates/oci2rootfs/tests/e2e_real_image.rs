#![cfg(feature = "remote")]

// End-to-end tests that pull real Docker images from registries, convert
// them to ext4 rootfs images, and verify the contents.
//
// These tests require network access and are marked #[ignore] so they don't
// run in normal `cargo test`.  Run them explicitly with:
//
//   cargo test --test e2e_real_image -- --ignored --nocapture
//
// Or run a specific one:
//
//   cargo test --test e2e_real_image test_alpine -- --ignored --nocapture

use arcbox_ext4::Reader;
use arcbox_ext4::constants::*;
use oci2rootfs::{Converter, Platform, RemoteRef};
use tempfile::NamedTempFile;

async fn convert_remote_image(
    output: &NamedTempFile,
    reference: &str,
    platform: Platform,
    size: u64,
) {
    let source = RemoteRef::new(reference)
        .platform(platform)
        .fetch()
        .await
        .unwrap();

    Converter::new(output.path())
        .size(size)
        .convert(source)
        .unwrap();
}

// ---------------------------------------------------------------------------
// Alpine Linux (tiny, ~3 MiB, single layer)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_alpine_amd64() {
    let tmp = NamedTempFile::new().unwrap();
    convert_remote_image(
        &tmp,
        "alpine:3.21",
        Platform::new("linux", "amd64"),
        128 * 1024 * 1024,
    )
    .await;

    let mut reader = Reader::new(tmp.path()).expect("failed to open ext4 image");

    // -- Superblock sanity --
    let sb = reader.superblock();
    assert_eq!(sb.magic, SUPERBLOCK_MAGIC);
    assert_eq!(sb.log_block_size, 2); // 4096 byte blocks

    // -- Core directory structure --
    assert!(reader.exists("/bin"));
    assert!(reader.exists("/etc"));
    assert!(reader.exists("/lib"));
    assert!(reader.exists("/usr"));
    assert!(reader.exists("/var"));
    assert!(reader.exists("/tmp"));
    assert!(reader.exists("/root"));
    assert!(reader.exists("/home"));
    assert!(reader.exists("/proc"));
    assert!(reader.exists("/sys"));
    assert!(reader.exists("/dev"));

    // -- /etc/alpine-release --
    assert!(reader.exists("/etc/alpine-release"));
    let release = reader.read_file("/etc/alpine-release", 0, None).unwrap();
    let release_str = String::from_utf8_lossy(&release);
    assert!(
        release_str.starts_with("3."),
        "expected alpine release 3.x, got: {release_str}"
    );

    // -- /etc/os-release --
    assert!(reader.exists("/etc/os-release"));
    let os_release = reader.read_file("/etc/os-release", 0, None).unwrap();
    let os_release_str = String::from_utf8_lossy(&os_release);
    assert!(
        os_release_str.contains("Alpine"),
        "/etc/os-release should mention Alpine: {os_release_str}"
    );

    // -- /etc/passwd --
    assert!(reader.exists("/etc/passwd"));
    let passwd = reader.read_file("/etc/passwd", 0, None).unwrap();
    let passwd_str = String::from_utf8_lossy(&passwd);
    assert!(
        passwd_str.contains("root:"),
        "/etc/passwd should have root entry"
    );

    // -- /bin/busybox (the core binary) --
    assert!(reader.exists("/bin/busybox"));
    let (_, busybox_inode) = reader.stat("/bin/busybox").unwrap();
    assert!(
        is_reg(busybox_inode.mode),
        "/bin/busybox should be a regular file"
    );
    assert!(
        busybox_inode.file_size() > 0,
        "/bin/busybox should not be empty"
    );

    // -- /bin/sh is typically a symlink to busybox --
    assert!(reader.exists("/bin/sh"));

    // -- list /etc and verify it has reasonable content --
    let etc_entries = reader.list_dir("/etc").unwrap();
    assert!(
        etc_entries.len() > 5,
        "/etc should have many entries, got: {}",
        etc_entries.len()
    );
    assert!(etc_entries.contains(&"passwd".to_string()));
    assert!(etc_entries.contains(&"group".to_string()));

    eprintln!("✓ alpine:3.21 amd64 -- all checks passed");
    eprintln!("  /etc has {} entries", etc_entries.len());
    eprintln!("  /bin/busybox size: {} bytes", busybox_inode.file_size());
}

// ---------------------------------------------------------------------------
// Alpine Linux ARM64 (tests multi-arch resolution)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_alpine_arm64() {
    let tmp = NamedTempFile::new().unwrap();
    convert_remote_image(
        &tmp,
        "alpine:3.21",
        Platform::new("linux", "arm64"),
        128 * 1024 * 1024,
    )
    .await;

    let mut reader = Reader::new(tmp.path()).expect("failed to open ext4 image");
    assert!(reader.exists("/bin/busybox"));
    assert!(reader.exists("/etc/alpine-release"));

    eprintln!("✓ alpine:3.21 arm64 -- basic checks passed");
}

// ---------------------------------------------------------------------------
// Ubuntu 22.04 (multi-layer, larger image ~28 MiB)
//
// NOTE: May fail with an ImageConfig parsing error in the `containerregistry`
// crate if the manifest contains null fields.  This is NOT an ext4 bug.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_ubuntu_2204_amd64() {
    let tmp = NamedTempFile::new().unwrap();
    convert_remote_image(
        &tmp,
        "ubuntu:22.04",
        Platform::new("linux", "amd64"),
        256 * 1024 * 1024,
    )
    .await;

    let mut reader = Reader::new(tmp.path()).expect("failed to open ext4 image");

    // -- Superblock --
    let sb = reader.superblock();
    assert_eq!(sb.magic, SUPERBLOCK_MAGIC);

    // -- Core Ubuntu directories --
    assert!(reader.exists("/bin"));
    assert!(reader.exists("/etc"));
    assert!(reader.exists("/lib"));
    assert!(reader.exists("/usr"));
    assert!(reader.exists("/var"));
    assert!(reader.exists("/tmp"));
    assert!(reader.exists("/root"));
    assert!(reader.exists("/home"));

    // -- /etc/os-release should identify Ubuntu --
    assert!(reader.exists("/etc/os-release"));
    let os_release = reader.read_file("/etc/os-release", 0, None).unwrap();
    let os_release_str = String::from_utf8_lossy(&os_release);
    assert!(
        os_release_str.contains("Ubuntu"),
        "/etc/os-release should mention Ubuntu: {os_release_str}"
    );
    assert!(
        os_release_str.contains("22.04") || os_release_str.contains("Jammy"),
        "/etc/os-release should reference 22.04/Jammy: {os_release_str}"
    );

    // -- /etc/passwd --
    let passwd = reader.read_file("/etc/passwd", 0, None).unwrap();
    let passwd_str = String::from_utf8_lossy(&passwd);
    assert!(passwd_str.contains("root:x:0:0:"));
    assert!(passwd_str.contains("nobody:"));

    // -- /etc/apt/sources.list or sources.list.d (Ubuntu has apt) --
    let has_apt =
        reader.exists("/etc/apt/sources.list") || reader.exists("/etc/apt/sources.list.d");
    assert!(has_apt, "Ubuntu should have apt sources configuration");

    // -- /usr/bin/dpkg (package manager) --
    assert!(reader.exists("/usr/bin/dpkg"), "Ubuntu should have dpkg");
    let (_, dpkg_inode) = reader.stat("/usr/bin/dpkg").unwrap();
    assert!(dpkg_inode.file_size() > 0);

    // -- Symlinks: /bin is often a symlink to /usr/bin on modern Ubuntu --
    // Check that symlink resolution works for deep paths.
    if reader.exists("/usr/bin/apt") {
        let (_, apt_inode) = reader.stat("/usr/bin/apt").unwrap();
        assert!(is_reg(apt_inode.mode) || is_link(apt_inode.mode));
    }

    // -- /var/lib/dpkg/status (dpkg database, proves layers applied correctly) --
    assert!(
        reader.exists("/var/lib/dpkg/status"),
        "dpkg status file should exist (proves multi-layer apply worked)"
    );
    let dpkg_status = reader.read_file("/var/lib/dpkg/status", 0, None).unwrap();
    let dpkg_str = String::from_utf8_lossy(&dpkg_status);
    assert!(
        dpkg_str.contains("Package:"),
        "dpkg status should contain package entries"
    );

    // -- Overall size sanity --
    let etc_entries = reader.list_dir("/etc").unwrap();
    let usr_bin_entries = reader.list_dir("/usr/bin").unwrap();
    eprintln!("✓ ubuntu:22.04 amd64 -- all checks passed");
    eprintln!("  /etc has {} entries", etc_entries.len());
    eprintln!("  /usr/bin has {} entries", usr_bin_entries.len());
    eprintln!("  /usr/bin/dpkg size: {} bytes", dpkg_inode.file_size());
}

// ---------------------------------------------------------------------------
// Debian bookworm-slim (another multi-layer variant)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_debian_bookworm_slim() {
    let tmp = NamedTempFile::new().unwrap();
    convert_remote_image(
        &tmp,
        "debian:bookworm-slim",
        Platform::default(),
        256 * 1024 * 1024,
    )
    .await;

    let mut reader = Reader::new(tmp.path()).expect("failed to open ext4 image");

    assert!(reader.exists("/etc/debian_version"));
    let version = reader.read_file("/etc/debian_version", 0, None).unwrap();
    let version_str = String::from_utf8_lossy(&version);
    assert!(
        version_str.starts_with("12"),
        "expected Debian 12 (bookworm), got: {version_str}"
    );

    assert!(reader.exists("/etc/passwd"));
    assert!(reader.exists("/usr/bin/dpkg"));

    eprintln!("✓ debian:bookworm-slim -- all checks passed");
}

// ---------------------------------------------------------------------------
// busybox (minimal, single-layer, ~4 MiB, good for quick smoke test)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_busybox() {
    let tmp = NamedTempFile::new().unwrap();
    convert_remote_image(
        &tmp,
        "busybox:latest",
        Platform::default(),
        64 * 1024 * 1024,
    )
    .await;

    let mut reader = Reader::new(tmp.path()).expect("failed to open ext4 image");

    assert!(reader.exists("/bin"));
    assert!(reader.exists("/bin/sh"));
    assert!(reader.exists("/bin/busybox"));

    // busybox uses many symlinks/hardlinks in /bin
    let bin_entries = reader.list_dir("/bin").unwrap();
    assert!(
        bin_entries.len() > 50,
        "busybox /bin should have many entries (symlinks to busybox), got: {}",
        bin_entries.len()
    );

    eprintln!("✓ busybox:latest -- all checks passed");
    eprintln!("  /bin has {} entries", bin_entries.len());
}
