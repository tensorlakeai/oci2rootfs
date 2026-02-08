use std::io::Read;
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Compute sha256 hex digest of data.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Create a minimal tar archive containing a single file.
fn build_test_tar() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let data = b"hello from layer";
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder
        .append_data(&mut header, "hello.txt", &data[..])
        .unwrap();
    builder.into_inner().unwrap()
}

/// Gzip-compress data.
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::copy(&mut std::io::Cursor::new(data), &mut encoder).unwrap();
    encoder.finish().unwrap()
}

/// Write a blob to the OCI layout blobs directory.
fn write_blob(layout_dir: &Path, digest_hex: &str, data: &[u8]) {
    let blob_dir = layout_dir.join("blobs").join("sha256");
    std::fs::create_dir_all(&blob_dir).unwrap();
    std::fs::write(blob_dir.join(digest_hex), data).unwrap();
}

/// Create a minimal OCI Image Layout in a temp directory.
///
/// Returns (tempdir, layer_gzip_bytes) where layer is a gzipped tar.
fn create_oci_layout() -> (TempDir, Vec<u8>) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // 1. oci-layout file
    std::fs::write(
        root.join("oci-layout"),
        r#"{"imageLayoutVersion":"1.0.0"}"#,
    )
    .unwrap();

    // 2. Build a layer: tar -> gzip
    let tar_data = build_test_tar();
    let layer_gz = gzip(&tar_data);
    let layer_digest = sha256_hex(&layer_gz);
    let uncompressed_digest = sha256_hex(&tar_data);

    // 3. Build config JSON
    let config_json = serde_json_config(&uncompressed_digest);
    let config_bytes = config_json.as_bytes().to_vec();
    let config_digest = sha256_hex(&config_bytes);

    // 4. Build manifest JSON
    let manifest_json = serde_json_manifest(&config_digest, config_bytes.len(), &layer_digest, layer_gz.len());
    let manifest_bytes = manifest_json.as_bytes().to_vec();
    let manifest_digest = sha256_hex(&manifest_bytes);

    // 5. Build index.json
    let index_json = serde_json_index(&manifest_digest, manifest_bytes.len());
    std::fs::write(root.join("index.json"), &index_json).unwrap();

    // 6. Write blobs
    write_blob(root, &layer_digest, &layer_gz);
    write_blob(root, &config_digest, &config_bytes);
    write_blob(root, &manifest_digest, &manifest_bytes);

    (dir, layer_gz)
}

fn serde_json_config(uncompressed_layer_digest: &str) -> String {
    format!(
        r#"{{"architecture":"amd64","os":"linux","rootfs":{{"type":"layers","diff_ids":["sha256:{uncompressed_layer_digest}"]}}}}"#
    )
}

fn serde_json_manifest(
    config_digest: &str,
    config_size: usize,
    layer_digest: &str,
    layer_size: usize,
) -> String {
    format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:{config_digest}","size":{config_size}}},"layers":[{{"mediaType":"application/vnd.oci.image.layer.v1.tar+gzip","digest":"sha256:{layer_digest}","size":{layer_size}}}]}}"#
    )
}

fn serde_json_index(manifest_digest: &str, manifest_size: usize) -> String {
    format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{manifest_digest}","size":{manifest_size}}}]}}"#
    )
}

#[test]
fn test_resolve_single_manifest() {
    let (dir, _layer_gz) = create_oci_layout();
    let image = oci2rootfs::oci::resolve(dir.path()).unwrap();
    assert_eq!(image.layers.len(), 1);
    assert_eq!(image.config.architecture, "amd64");
    assert_eq!(image.config.os, "linux");
}

#[test]
fn test_resolve_layer_media_type() {
    let (dir, _layer_gz) = create_oci_layout();
    let image = oci2rootfs::oci::resolve(dir.path()).unwrap();
    let layer = &image.layers[0];
    assert_eq!(
        layer.media_type.as_str(),
        "application/vnd.oci.image.layer.v1.tar+gzip"
    );
}

#[test]
fn test_open_layer_gzip_readable() {
    let (dir, _layer_gz) = create_oci_layout();
    let image = oci2rootfs::oci::resolve(dir.path()).unwrap();
    let layer = &image.layers[0];

    // open_layer should return a GzDecoder that yields the original tar
    let mut reader = image.open_layer(layer).unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).unwrap();

    // The decompressed output should be a valid tar containing hello.txt
    let mut archive = tar::Archive::new(std::io::Cursor::new(buf));
    let entries: Vec<_> = archive.entries().unwrap().collect();
    assert!(!entries.is_empty());
}

#[test]
fn test_resolve_invalid_path() {
    let result = oci2rootfs::oci::resolve("/nonexistent/oci/layout");
    assert!(result.is_err());
}

#[test]
fn test_open_layer_uncompressed() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create layout with uncompressed layer
    std::fs::write(
        root.join("oci-layout"),
        r#"{"imageLayoutVersion":"1.0.0"}"#,
    )
    .unwrap();

    let tar_data = build_test_tar();
    let layer_digest = sha256_hex(&tar_data);
    let uncompressed_digest = sha256_hex(&tar_data); // same since not compressed

    let config_json = serde_json_config(&uncompressed_digest);
    let config_bytes = config_json.as_bytes().to_vec();
    let config_digest = sha256_hex(&config_bytes);

    // Use uncompressed media type
    let manifest_json = format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:{config_digest}","size":{}}},"layers":[{{"mediaType":"application/vnd.oci.image.layer.v1.tar","digest":"sha256:{layer_digest}","size":{}}}]}}"#,
        config_bytes.len(),
        tar_data.len()
    );
    let manifest_bytes = manifest_json.as_bytes().to_vec();
    let manifest_digest = sha256_hex(&manifest_bytes);

    let index_json = serde_json_index(&manifest_digest, manifest_bytes.len());
    std::fs::write(root.join("index.json"), &index_json).unwrap();

    write_blob(root, &layer_digest, &tar_data);
    write_blob(root, &config_digest, &config_bytes);
    write_blob(root, &manifest_digest, &manifest_bytes);

    let image = oci2rootfs::oci::resolve(root).unwrap();
    let layer = &image.layers[0];

    let mut reader = image.open_layer(layer).unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).unwrap();

    // Should be identical to original tar data
    assert_eq!(buf, tar_data);
}
