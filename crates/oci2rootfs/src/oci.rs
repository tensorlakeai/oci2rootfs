use containerregistry_image::{Descriptor, Digest, ImageIndex};
use containerregistry_layout::Layout;

use crate::convert::Platform;
use crate::error::{Error, Result};
use crate::tar_source::TarImageSource;

/// Resolve an OCI image layout into a tar-layer-backed image source.
pub(crate) fn resolve(layout: Layout, platform: &Platform) -> Result<(TarImageSource, Digest)> {
    let index = layout.index()?;

    let manifest_desc = find_manifest_descriptor(&layout, &index, platform)?;
    let manifest = layout.read_manifest(&manifest_desc.digest)?;
    let config = layout.read_config(&manifest.config().digest)?;
    let layers: Vec<_> = manifest
        .layers()
        .iter()
        .map(|layer| (layer.clone(), layout.blob_path(&layer.digest)))
        .collect();

    tracing::info!(
        os = platform.os(),
        arch = platform.arch(),
        manifest_digest = %manifest_desc.digest,
        layer_count = layers.len(),
        "resolved OCI image layout"
    );

    Ok((
        TarImageSource::from_files(config, layers),
        manifest_desc.digest,
    ))
}

/// Find the best manifest descriptor from an image index.
fn find_manifest_descriptor(
    layout: &Layout,
    index: &ImageIndex,
    platform: &Platform,
) -> Result<Descriptor> {
    let descriptors = index.manifests();

    if descriptors.len() == 1 && !descriptors[0].media_type.is_index() {
        return Ok(descriptors[0].clone());
    }

    if let Some(desc) = index.find_platform(platform.arch(), platform.os(), None) {
        if desc.media_type.is_index() {
            let nested_data = layout.read_blob(&desc.digest)?;
            let nested = ImageIndex::from_bytes(&nested_data)?;
            return find_manifest_descriptor(layout, &nested, platform);
        }
        return Ok(desc.clone());
    }

    for desc in descriptors {
        if desc.media_type.is_index() {
            let nested_data = layout.read_blob(&desc.digest)?;
            let nested = ImageIndex::from_bytes(&nested_data)?;
            if let Ok(resolved) = find_manifest_descriptor(layout, &nested, platform) {
                return Ok(resolved);
            }
        }
    }

    for desc in descriptors {
        if desc.media_type.is_manifest() {
            return Ok(desc.clone());
        }
    }

    Err(Error::NoManifest(format!(
        "{}/{}",
        platform.os(),
        platform.arch()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::SourceImpl;
    use containerregistry_layout::Layout;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest as _, Sha256};
    use tempfile::TempDir;

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

    fn config_json(arch: &str, diff_id: &str) -> String {
        format!(
            r#"{{"architecture":"{arch}","os":"linux","rootfs":{{"type":"layers","diff_ids":["sha256:{diff_id}"]}}}}"#
        )
    }

    fn manifest_json(
        config_digest: &str,
        config_size: usize,
        layer_digest: &str,
        layer_size: usize,
    ) -> String {
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:{config_digest}","size":{config_size}}},"layers":[{{"mediaType":"application/vnd.oci.image.layer.v1.tar+gzip","digest":"sha256:{layer_digest}","size":{layer_size}}}]}}"#
        )
    }

    fn platform_index_entry(manifest_digest: &str, manifest_size: usize, arch: &str) -> String {
        format!(
            r#"{{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{manifest_digest}","size":{manifest_size},"platform":{{"os":"linux","architecture":"{arch}"}}}}"#
        )
    }

    fn create_single_manifest_layout() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::write(root.join("oci-layout"), r#"{"imageLayoutVersion":"1.0.0"}"#).unwrap();

        let tar_data = build_test_tar("hello.txt", b"hello from layer");
        let layer_gz = gzip(&tar_data);
        let layer_digest = sha256_hex(&layer_gz);
        let diff_id = sha256_hex(&tar_data);

        let config = config_json("amd64", &diff_id);
        let config_bytes = config.as_bytes().to_vec();
        let config_digest = sha256_hex(&config_bytes);

        let manifest = manifest_json(
            &config_digest,
            config_bytes.len(),
            &layer_digest,
            layer_gz.len(),
        );
        let manifest_bytes = manifest.as_bytes().to_vec();
        let manifest_digest = sha256_hex(&manifest_bytes);

        let index_json = format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{}]}}"#,
            platform_index_entry(&manifest_digest, manifest_bytes.len(), "amd64")
        );
        std::fs::write(root.join("index.json"), index_json).unwrap();

        write_blob(root, &layer_digest, &layer_gz);
        write_blob(root, &config_digest, &config_bytes);
        write_blob(root, &manifest_digest, &manifest_bytes);

        dir
    }

    fn create_multi_platform_layout() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::write(root.join("oci-layout"), r#"{"imageLayoutVersion":"1.0.0"}"#).unwrap();

        let mut entries = Vec::new();

        for arch in ["amd64", "arm64"] {
            let tar_data = build_test_tar(&format!("{arch}.txt"), arch.as_bytes());
            let layer_gz = gzip(&tar_data);
            let layer_digest = sha256_hex(&layer_gz);
            let diff_id = sha256_hex(&tar_data);

            let config = config_json(arch, &diff_id);
            let config_bytes = config.as_bytes().to_vec();
            let config_digest = sha256_hex(&config_bytes);

            let manifest = manifest_json(
                &config_digest,
                config_bytes.len(),
                &layer_digest,
                layer_gz.len(),
            );
            let manifest_bytes = manifest.as_bytes().to_vec();
            let manifest_digest = sha256_hex(&manifest_bytes);

            write_blob(root, &layer_digest, &layer_gz);
            write_blob(root, &config_digest, &config_bytes);
            write_blob(root, &manifest_digest, &manifest_bytes);

            entries.push(platform_index_entry(
                &manifest_digest,
                manifest_bytes.len(),
                arch,
            ));
        }

        let index_json = format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{}]}}"#,
            entries.join(",")
        );
        std::fs::write(root.join("index.json"), index_json).unwrap();

        dir
    }

    #[test]
    fn resolve_single_manifest() {
        let dir = create_single_manifest_layout();
        let layout = Layout::open(dir.path()).unwrap();
        let (image, _) = resolve(layout, &Platform::default()).unwrap();

        assert_eq!(image.layer_count(), 1);
        assert_eq!(image.config().architecture, "amd64");
        assert_eq!(image.config().os, "linux");
    }

    #[test]
    fn resolve_platform_override() {
        let dir = create_multi_platform_layout();
        let layout = Layout::open(dir.path()).unwrap();
        let (image, _) = resolve(layout, &Platform::new("linux", "arm64")).unwrap();

        assert_eq!(image.config().architecture, "arm64");
    }
}
