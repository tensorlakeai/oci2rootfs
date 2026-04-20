#![cfg(unix)]

use arcbox_ext4::Reader;
use flate2::Compression;
use flate2::write::GzEncoder;
use oci2rootfs::{
    Converter, Error, Ext4Options, IntoImageSource, OciLayoutSource, Overlay2Source, Platform,
    autodetect,
};
use serial_test::serial;
use sha2::{Digest as _, Sha256};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use tempfile::{NamedTempFile, TempDir};
use uuid::Uuid;

const TEST_SIZE: u64 = 32 * 1024 * 1024;

/// Create an overlay2 layer directory (`diff/` + `link`, plus optional
/// `lower`). Returns the chain-id directory.
fn create_overlay2_layer(
    overlay2_root: &Path,
    chain_id: &str,
    link_id: &str,
    lower: Option<&str>,
) -> std::path::PathBuf {
    let dir = overlay2_root.join(chain_id);
    std::fs::create_dir_all(dir.join("diff")).unwrap();
    std::fs::write(dir.join("link"), link_id).unwrap();
    if let Some(lower) = lower {
        std::fs::write(dir.join("lower"), lower).unwrap();
    }
    dir
}

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

/// Build a minimal OCI image layout with one gzipped tar layer containing
/// `hello.txt`. `container_config_fragment` is inserted as a `"config": { ... }`
/// member of the image config JSON when `Some` — callers can inject
/// entrypoint/cmd/env to assert propagation.
fn create_oci_layout_with(container_config_fragment: Option<&str>) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::write(root.join("oci-layout"), r#"{"imageLayoutVersion":"1.0.0"}"#).unwrap();

    let tar_data = build_test_tar("hello.txt", b"hello from oci");
    let layer_gz = gzip(&tar_data);
    let layer_digest = sha256_hex(&layer_gz);
    let diff_id = sha256_hex(&tar_data);

    let config_inner = container_config_fragment
        .map(|frag| format!(r#","config":{frag}"#))
        .unwrap_or_default();
    let config_json = format!(
        r#"{{"architecture":"amd64","os":"linux","rootfs":{{"type":"layers","diff_ids":["sha256:{diff_id}"]}}{config_inner}}}"#
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

fn create_oci_layout() -> TempDir {
    create_oci_layout_with(None)
}

#[test]
#[serial]
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
#[serial]
fn autodetect_overlay2_source() {
    let root = TempDir::new().unwrap();
    let layer = create_overlay2_layer(root.path(), "layer-a", "AAA", None);
    std::fs::write(layer.join("diff").join("hello.txt"), b"hello from overlay").unwrap();

    let output = NamedTempFile::new().unwrap();
    let source = autodetect(&layer).unwrap();
    assert_eq!(source.layer_count(), 1);

    Converter::new(output.path())
        .size(TEST_SIZE)
        .convert(source)
        .unwrap();

    let mut reader = Reader::new(output.path()).unwrap();
    assert!(reader.exists("/hello.txt"));
}

#[test]
#[serial]
fn overlay2_chain_applies_layers_bottom_to_top() {
    let overlay2_root = TempDir::new().unwrap();
    let root = overlay2_root.path();
    std::fs::create_dir_all(root.join("l")).unwrap();

    // Base: writes /keep.txt and /replace.txt (first value).
    let base = create_overlay2_layer(root, "base", "BASE", None);
    std::fs::write(base.join("diff").join("keep.txt"), b"base-keep").unwrap();
    std::fs::write(base.join("diff").join("replace.txt"), b"base-replace").unwrap();
    std::os::unix::fs::symlink("../base/diff", root.join("l/BASE")).unwrap();

    // Top: overrides /replace.txt and adds /new.txt.
    let top = create_overlay2_layer(root, "top", "TOP", Some("l/BASE"));
    std::fs::write(top.join("diff").join("replace.txt"), b"top-replace").unwrap();
    std::fs::write(top.join("diff").join("new.txt"), b"top-new").unwrap();

    let output = NamedTempFile::new().unwrap();
    Converter::new(output.path())
        .size(TEST_SIZE)
        .convert(Overlay2Source::open(&top).unwrap())
        .unwrap();

    let mut reader = Reader::new(output.path()).unwrap();
    assert_eq!(
        reader.read_file("/keep.txt", 0, None).unwrap(),
        b"base-keep"
    );
    assert_eq!(
        reader.read_file("/replace.txt", 0, None).unwrap(),
        b"top-replace"
    );
    assert_eq!(reader.read_file("/new.txt", 0, None).unwrap(), b"top-new");
}

#[test]
#[serial]
fn overlay2_oci_whiteout_deletes_lower_entry() {
    let overlay2_root = TempDir::new().unwrap();
    let root = overlay2_root.path();
    std::fs::create_dir_all(root.join("l")).unwrap();

    let base = create_overlay2_layer(root, "base", "BASE", None);
    std::fs::write(base.join("diff").join("doomed.txt"), b"bye").unwrap();
    std::os::unix::fs::symlink("../base/diff", root.join("l/BASE")).unwrap();

    let top = create_overlay2_layer(root, "top", "TOP", Some("l/BASE"));
    // OCI-style whiteout: `.wh.<name>` at the same parent.
    std::fs::write(top.join("diff").join(".wh.doomed.txt"), b"").unwrap();

    let output = NamedTempFile::new().unwrap();
    Converter::new(output.path())
        .size(TEST_SIZE)
        .convert(Overlay2Source::open(&top).unwrap())
        .unwrap();

    let mut reader = Reader::new(output.path()).unwrap();
    assert!(!reader.exists("/doomed.txt"));
}

#[test]
#[serial]
fn overlay2_hardlink_dedup_preserves_inode_sharing() {
    let overlay2_root = TempDir::new().unwrap();
    let layer = create_overlay2_layer(overlay2_root.path(), "layer", "LAYER", None);
    let diff = layer.join("diff");
    let original = diff.join("original.bin");
    let linked = diff.join("linked.bin");
    std::fs::write(&original, b"shared payload").unwrap();
    std::fs::hard_link(&original, &linked).unwrap();
    // Sanity check: host sees shared inode.
    assert_eq!(
        std::fs::metadata(&original).unwrap().ino(),
        std::fs::metadata(&linked).unwrap().ino()
    );

    let output = NamedTempFile::new().unwrap();
    Converter::new(output.path())
        .size(TEST_SIZE)
        .convert(Overlay2Source::open(&layer).unwrap())
        .unwrap();

    let mut reader = Reader::new(output.path()).unwrap();
    let (ino_a, inode_a) = reader.stat("/original.bin").unwrap();
    let (ino_b, inode_b) = reader.stat("/linked.bin").unwrap();
    assert_eq!(
        ino_a, ino_b,
        "hardlinked host files should share an inode in ext4"
    );
    assert_eq!(inode_a.links_count, 2);
    assert_eq!(inode_b.links_count, 2);
}

#[test]
#[serial]
fn image_config_exposed_for_oci_layout() {
    let container_config = r#"{
        "User": "0:0",
        "Entrypoint": ["/bin/sh", "-c"],
        "Cmd": ["echo hello"],
        "Env": ["PATH=/usr/local/bin:/usr/bin:/bin", "LANG=en_US.UTF-8"],
        "WorkingDir": "/app"
    }"#;
    let layout = create_oci_layout_with(Some(container_config));

    let source = OciLayoutSource::open(layout.path())
        .unwrap()
        .into_image_source()
        .unwrap();

    let cfg = source.config().expect("OCI layout source carries config");
    let container = cfg.config.as_ref().expect("container config present");

    assert_eq!(container.user.as_deref(), Some("0:0"));
    assert_eq!(container.entrypoint.as_deref().unwrap(), &["/bin/sh", "-c"]);
    assert_eq!(container.cmd.as_deref().unwrap(), &["echo hello"]);
    assert!(container.env.iter().any(|e| e.starts_with("PATH=")));
    assert_eq!(container.working_dir.as_deref(), Some("/app"));
}

#[test]
#[serial]
fn image_config_none_for_overlay2() {
    let root = TempDir::new().unwrap();
    let layer = create_overlay2_layer(root.path(), "only", "ONLY", None);
    std::fs::write(layer.join("diff").join("f"), b"x").unwrap();
    let source = Overlay2Source::open(&layer)
        .unwrap()
        .into_image_source()
        .unwrap();
    assert!(source.config().is_none());
}

#[test]
#[serial]
fn preflight_rejects_too_small_image() {
    let layout = create_oci_layout();
    let output = NamedTempFile::new().unwrap();
    // Estimate is always >= 16 MiB metadata overhead; a 1 MiB ceiling must fail.
    let result = Converter::new(output.path())
        .size(1024 * 1024)
        .convert(OciLayoutSource::open(layout.path()).unwrap());
    match result {
        Err(Error::InsufficientSize { needed, configured }) => {
            assert!(needed > configured, "expected needed > configured");
            assert_eq!(configured, 1024 * 1024);
        }
        other => panic!("expected InsufficientSize, got {other:?}"),
    }
    // Preflight rejects BEFORE the file would be formatted: the inherited
    // NamedTempFile creates an empty file at output.path() already, so we
    // only assert that size is still zero (nothing was written to it).
    assert_eq!(std::fs::metadata(output.path()).unwrap().len(), 0);
}

#[test]
#[serial]
fn partial_output_cleaned_on_apply_failure() {
    // Corrupt an OCI layout by replacing its gzipped layer blob with
    // garbage. preflight passes (it only reads manifest sizes), but
    // `apply_layer` errors once it tries to decompress.
    let layout = create_oci_layout();
    let layer_blob = std::fs::read_dir(layout.path().join("blobs").join("sha256"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| std::fs::metadata(p).unwrap().len() > 100)
        .expect("find a layer-sized blob");
    std::fs::write(&layer_blob, b"not a valid gzip stream").unwrap();

    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("rootfs.ext4");
    assert!(!output.exists());
    let result = Converter::new(&output)
        .size(TEST_SIZE)
        .convert(OciLayoutSource::open(layout.path()).unwrap());

    assert!(result.is_err(), "expected apply to fail");
    assert!(
        !output.exists(),
        "partial output at {} should have been removed",
        output.display()
    );
}

#[test]
#[serial]
fn ext4_options_label_and_uuid_written_to_superblock() {
    let layout = create_oci_layout();
    let output = NamedTempFile::new().unwrap();

    let target_uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
    Converter::new(output.path())
        .size(TEST_SIZE)
        .ext4_options(Ext4Options::new().label("alpine-boot").uuid(target_uuid))
        .convert(OciLayoutSource::open(layout.path()).unwrap())
        .unwrap();

    let reader = Reader::new(output.path()).unwrap();
    let sb = reader.superblock();

    let label = std::str::from_utf8(&sb.volume_name)
        .unwrap()
        .trim_end_matches('\0');
    assert_eq!(label, "alpine-boot");
    assert_eq!(sb.uuid, *target_uuid.as_bytes());
}

#[test]
#[serial]
fn ext4_options_reject_oversized_label() {
    let layout = create_oci_layout();
    let output = NamedTempFile::new().unwrap();

    let result = Converter::new(output.path())
        .size(TEST_SIZE)
        .ext4_options(Ext4Options::new().label("this-label-is-way-too-long"))
        .convert(OciLayoutSource::open(layout.path()).unwrap());

    assert!(matches!(result, Err(Error::InvalidLabel(_))));
}
