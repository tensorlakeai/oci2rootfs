use arcbox_ext4::Reader;
use flate2::Compression;
use flate2::write::GzEncoder;
use oci2rootfs::{Converter, OciLayoutSource, Platform, autodetect};
use sha2::{Digest as _, Sha256};
use tempfile::{NamedTempFile, TempDir};

const TEST_SIZE: u64 = 16 * 1024 * 1024;

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn build_test_tar(file_name: &str, contents: &[u8]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder
        .append_data(&mut header, file_name, contents)
        .unwrap();
    builder.into_inner().unwrap()
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::copy(&mut std::io::Cursor::new(data), &mut encoder).unwrap();
    encoder.finish().unwrap()
}

fn write_blob(layout_dir: &std::path::Path, digest_hex: &str, data: &[u8]) {
    let blob_dir = layout_dir.join("blobs").join("sha256");
    std::fs::create_dir_all(&blob_dir).unwrap();
    std::fs::write(blob_dir.join(digest_hex), data).unwrap();
}

fn create_oci_layout() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::write(root.join("oci-layout"), r#"{"imageLayoutVersion":"1.0.0"}"#).unwrap();

    let tar_data = build_test_tar("hello.txt", b"hello from oci");
    let layer_gz = gzip(&tar_data);
    let layer_digest = sha256_hex(&layer_gz);
    let diff_id = sha256_hex(&tar_data);

    let config_json = format!(
        r#"{{"architecture":"amd64","os":"linux","rootfs":{{"type":"layers","diff_ids":["sha256:{diff_id}"]}}}}"#
    );
    let config_bytes = config_json.as_bytes().to_vec();
    let config_digest = sha256_hex(&config_bytes);

    let manifest_json = format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:{config_digest}","size":{}}},"layers":[{{"mediaType":"application/vnd.oci.image.layer.v1.tar+gzip","digest":"sha256:{layer_digest}","size":{}}}]}}"#,
        config_bytes.len(),
        layer_gz.len()
    );
    let manifest_bytes = manifest_json.as_bytes().to_vec();
    let manifest_digest = sha256_hex(&manifest_bytes);

    let index_json = format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{manifest_digest}","size":{}}}]}}"#,
        manifest_bytes.len()
    );
    std::fs::write(root.join("index.json"), index_json).unwrap();

    write_blob(root, &layer_digest, &layer_gz);
    write_blob(root, &config_digest, &config_bytes);
    write_blob(root, &manifest_digest, &manifest_bytes);

    dir
}

#[test]
fn convert_oci_layout_source() {
    let layout = create_oci_layout();
    let output = NamedTempFile::new().unwrap();

    Converter::new(output.path())
        .size(TEST_SIZE)
        .convert(
            OciLayoutSource::open(layout.path())
                .unwrap()
                .platform(Platform::default()),
        )
        .unwrap();

    let mut reader = Reader::new(output.path()).unwrap();
    assert!(reader.exists("/hello.txt"));
}

#[test]
fn autodetect_overlay2_source() {
    let root = TempDir::new().unwrap();
    let layer = root.path().join("layer-a");
    std::fs::create_dir_all(layer.join("diff")).unwrap();
    std::fs::write(layer.join("diff").join("hello.txt"), b"hello from overlay").unwrap();
    std::fs::write(layer.join("link"), "AAA").unwrap();

    let output = NamedTempFile::new().unwrap();
    let source = autodetect(&layer, Platform::default()).unwrap();
    assert_eq!(source.layer_count(), 1);

    Converter::new(output.path())
        .size(TEST_SIZE)
        .convert(source)
        .unwrap();

    let mut reader = Reader::new(output.path()).unwrap();
    assert!(reader.exists("/hello.txt"));
}
