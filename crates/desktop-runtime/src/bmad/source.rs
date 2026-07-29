//! Source-package boundary for the BMAD runtime.
//!
//! This service seals untrusted bytes into a generic source-domain value. It
//! deliberately knows nothing about Help, invocation adaptation, or I/O.

use super::{
    kernel_error::BmadKernelError,
    package::{BmadSourceEntry, BmadSourceSnapshot},
};

/// Generic, sealed source package. Capability-specific adapters consume its
/// snapshot outside this domain.
#[derive(Clone, Debug)]
pub struct BmadSourcePackage {
    snapshot: BmadSourceSnapshot,
}

impl BmadSourcePackage {
    #[must_use]
    pub const fn snapshot(&self) -> &BmadSourceSnapshot {
        &self.snapshot
    }
}

/// Pure source service used behind the runtime facade.
pub struct BmadSourceService;

impl BmadSourceService {
    /// Seals observed source entries into a deterministic snapshot.
    ///
    /// # Errors
    ///
    /// Returns a stable domain error for bounds or path-alias violations.
    /// Validates and loads source entries into a generic package.
    ///
    /// # Errors
    ///
    /// Returns a stable domain error for bounds, unsafe paths, or aliases.
    pub fn load(entries: Vec<BmadSourceEntry>) -> Result<BmadSourcePackage, BmadKernelError> {
        Ok(BmadSourcePackage {
            snapshot: BmadSourceSnapshot::new(entries)?,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    use crate::sha256_bytes;

    use super::super::package::{BmadLocationClass, BmadSourceKind};

    fn entry(path: &str, bytes: &[u8]) -> Result<BmadSourceEntry, BmadKernelError> {
        BmadSourceEntry::new(
            path,
            bytes.to_vec(),
            BmadSourceKind::SealedFoundation,
            BmadLocationClass::ManagedMetadata,
        )
    }

    #[test]
    fn source_load_returns_a_generic_sorted_package() {
        let package = BmadSourceService::load(vec![
            entry("z.json", b"z").expect("z"),
            entry("a.json", b"a").expect("a"),
        ])
        .expect("generic package");
        let paths = package
            .snapshot()
            .entries()
            .iter()
            .map(BmadSourceEntry::path)
            .collect::<Vec<_>>();
        assert_eq!(paths, ["a.json", "z.json"]);
        assert_eq!(
            package.snapshot().entries()[0].content_hash(),
            sha256_bytes(b"a")
        );
    }

    #[test]
    fn source_load_error_matrix_rejects_unsafe_and_aliasing_inputs() {
        for path in ["/root.json", "../escape.json", "a\\b.json"] {
            assert!(entry(path, b"x").is_err(), "{path} must fail");
        }
        let result = BmadSourceService::load(vec![
            entry("Data/File.json", b"a").expect("upper"),
            entry("data/file.json", b"b").expect("lower"),
        ]);
        assert!(result.is_err());
    }
}
