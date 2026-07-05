use rkyv::{Archive, Deserialize, Serialize};

use crate::WorkspaceIndex;

pub const ARCHIVE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct ArchivedWorkspace {
    pub schema_version: u32,
    pub index: WorkspaceIndex,
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("workspace archive schema {found} is not supported; expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
}

impl ArchivedWorkspace {
    pub fn new(index: WorkspaceIndex) -> Self {
        Self {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            index,
        }
    }

    pub fn validate_schema(&self) -> Result<(), ArchiveError> {
        if self.schema_version == ARCHIVE_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(ArchiveError::UnsupportedSchema {
                found: self.schema_version,
                expected: ARCHIVE_SCHEMA_VERSION,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ARCHIVE_SCHEMA_VERSION, ArchivedWorkspace};
    use crate::WorkspaceIndex;

    #[test]
    fn rejects_stale_schema() {
        let archive = ArchivedWorkspace {
            schema_version: ARCHIVE_SCHEMA_VERSION + 1,
            index: WorkspaceIndex {
                diagnostics: Vec::new(),
                imports: Vec::new(),
                symbols: Vec::new(),
            },
        };

        assert!(archive.validate_schema().is_err());
    }

    #[test]
    fn rkyv_round_trip_workspace_archive() {
        let archive = ArchivedWorkspace::new(WorkspaceIndex {
            diagnostics: Vec::new(),
            imports: Vec::new(),
            symbols: Vec::new(),
        });
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&archive).unwrap();
        let archived =
            rkyv::access::<super::ArchivedArchivedWorkspace, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(archived.schema_version, ARCHIVE_SCHEMA_VERSION);
    }
}
