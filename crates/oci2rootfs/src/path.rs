//! Path helpers shared between the tar-layer and overlay2-directory appliers.
//!
//! Both code paths walk a tree of entries, compute an absolute ext4 path for
//! each one, and classify `.wh.*` / `.wh..wh..opq` markers per the OCI
//! image-spec whiteout convention. These helpers centralize that logic.

use std::path::{Component, Path};

use crate::error::{Error, Result};

/// An OCI whiteout marker detected on a tar entry name or a directory-walk leaf.
///
/// Naming note: `Delete` carries just the leaf name; callers combine it with
/// the parent directory via [`join`].
pub(crate) enum Whiteout<'a> {
    /// `.wh.<name>` — delete `<parent>/<name>`.
    Delete(&'a str),
    /// `.wh..wh..opq` — clear every entry in the parent directory.
    Opaque,
}

/// Classify a leaf file name as an OCI whiteout marker.
pub(crate) fn parse_oci_whiteout(file_name: &str) -> Option<Whiteout<'_>> {
    if file_name == ".wh..wh..opq" {
        return Some(Whiteout::Opaque);
    }
    file_name
        .strip_prefix(".wh.")
        .filter(|name| !name.is_empty())
        .map(Whiteout::Delete)
}

/// Parent of an absolute ext4 path, keeping `/` for root and root-children.
pub(crate) fn parent_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(pos) => trimmed[..pos].to_string(),
    }
}

/// Join an absolute directory path with a leaf name, preserving a single `/`.
pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir == "/" {
        format!("/{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// Validate a tar entry path and canonicalize it to an absolute ext4 path.
///
/// Uses [`Path::components`] — `ParentDir` components are rejected outright,
/// `CurDir` is skipped, `RootDir`/`Prefix` collapse into the leading `/`, and
/// `Normal` segments are joined. NUL bytes are rejected. Empty paths normalize
/// to `/`.
pub(crate) fn sanitize_entry_path(raw: &Path) -> Result<String> {
    let mut out = String::from("/");
    for comp in raw.components() {
        match comp {
            Component::Normal(seg) => {
                let seg = seg.to_str().ok_or_else(|| {
                    Error::InvalidTarPath(format!("non-UTF-8 path component: {}", raw.display()))
                })?;
                if seg.contains('\0') {
                    return Err(Error::InvalidTarPath(format!(
                        "NUL byte in tar entry path: {}",
                        raw.display()
                    )));
                }
                if out.len() > 1 {
                    out.push('/');
                }
                out.push_str(seg);
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            Component::ParentDir => {
                return Err(Error::InvalidTarPath(format!(
                    "parent-dir component in tar entry: {}",
                    raw.display()
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_accepts_absolute() {
        assert_eq!(
            sanitize_entry_path(Path::new("/usr/bin/foo")).unwrap(),
            "/usr/bin/foo"
        );
    }

    #[test]
    fn sanitize_accepts_relative() {
        assert_eq!(
            sanitize_entry_path(Path::new("usr/bin/foo")).unwrap(),
            "/usr/bin/foo"
        );
        assert_eq!(
            sanitize_entry_path(Path::new("./usr/bin/foo")).unwrap(),
            "/usr/bin/foo"
        );
    }

    #[test]
    fn sanitize_trims_trailing_slash() {
        assert_eq!(sanitize_entry_path(Path::new("/usr/")).unwrap(), "/usr");
        assert_eq!(sanitize_entry_path(Path::new("./etc/")).unwrap(), "/etc");
    }

    #[test]
    fn sanitize_empty_becomes_root() {
        assert_eq!(sanitize_entry_path(Path::new("")).unwrap(), "/");
        assert_eq!(sanitize_entry_path(Path::new(".")).unwrap(), "/");
        assert_eq!(sanitize_entry_path(Path::new("./")).unwrap(), "/");
    }

    #[test]
    fn sanitize_rejects_parent_dir() {
        let err = sanitize_entry_path(Path::new("../etc/passwd")).unwrap_err();
        assert!(matches!(err, Error::InvalidTarPath(_)));
    }

    #[test]
    fn sanitize_rejects_nested_parent_dir() {
        let err = sanitize_entry_path(Path::new("etc/../passwd")).unwrap_err();
        assert!(matches!(err, Error::InvalidTarPath(_)));
    }

    #[test]
    fn sanitize_rejects_nul_byte() {
        let err = sanitize_entry_path(Path::new("etc/foo\0bar")).unwrap_err();
        assert!(matches!(err, Error::InvalidTarPath(_)));
    }

    #[test]
    fn parse_oci_whiteout_variants() {
        assert!(matches!(
            parse_oci_whiteout(".wh.resolv.conf"),
            Some(Whiteout::Delete("resolv.conf"))
        ));
        assert!(matches!(
            parse_oci_whiteout(".wh..wh..opq"),
            Some(Whiteout::Opaque)
        ));
        assert!(parse_oci_whiteout("passwd").is_none());
        assert!(parse_oci_whiteout(".wh.").is_none()); // empty name is not a valid whiteout
    }

    #[test]
    fn parent_of_cases() {
        assert_eq!(parent_of("/"), "/");
        assert_eq!(parent_of("/foo"), "/");
        assert_eq!(parent_of("/a/b/c"), "/a/b");
        assert_eq!(parent_of("/a/b/"), "/a");
    }

    #[test]
    fn join_cases() {
        assert_eq!(join("/", "foo"), "/foo");
        assert_eq!(join("/etc", "passwd"), "/etc/passwd");
    }
}
