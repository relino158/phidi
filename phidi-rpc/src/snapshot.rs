//! Typed RPC records for snapshot status and schema negotiation.
//!
//! These messages intentionally mirror the snapshot schema/freshness semantics
//! used elsewhere in the workspace while remaining self-contained, so
//! `phidi-rpc` can expose the wire contract without introducing a crate cycle.

use serde::{Deserialize, Serialize};

/// The newest snapshot schema this build knows how to negotiate over RPC.
pub const CURRENT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// The oldest snapshot schema this build promises to read over RPC.
pub const MINIMUM_READABLE_SCHEMA_VERSION: SchemaVersion = CURRENT_SCHEMA_VERSION;

/// Snapshot schema version encoded as explicit major/minor fields.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct SchemaVersion {
    /// Breaking schema line. Readers require the same major version.
    pub major: u16,
    /// Backward-compatible revision within a major schema line.
    pub minor: u16,
}

impl SchemaVersion {
    /// Creates a schema version.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Compares this version against the supported range for a reader.
    pub const fn compatibility(
        self,
        current: SchemaVersion,
        minimum: SchemaVersion,
    ) -> SchemaCompatibility {
        if self.major != current.major {
            return if self.major < current.major {
                SchemaCompatibility::TooOld
            } else {
                SchemaCompatibility::TooNew
            };
        }

        if self.minor > current.minor {
            SchemaCompatibility::TooNew
        } else if self.minor < minimum.minor {
            SchemaCompatibility::TooOld
        } else if self.minor == current.minor {
            SchemaCompatibility::Current
        } else {
            SchemaCompatibility::Compatible
        }
    }

    /// Compares this version against the support policy compiled into the current build.
    pub const fn compatibility_with_current(self) -> SchemaCompatibility {
        self.compatibility(CURRENT_SCHEMA_VERSION, MINIMUM_READABLE_SCHEMA_VERSION)
    }
}

/// Result of comparing a snapshot schema version with the current reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SchemaCompatibility {
    /// Matches the exact schema version produced by this build.
    Current,
    /// Older but still readable without migration.
    Compatible,
    /// Older than the minimum readable version for this build.
    TooOld,
    /// Newer than the reader understands.
    TooNew,
}

/// How closely a snapshot matches the current workspace state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotFreshness {
    /// Schema and workspace state match exactly.
    Exact,
    /// Schema is compatible and the revision matches, but the working tree is dirty.
    Drifted,
    /// Schema is compatible, but the snapshot was built from older workspace content.
    Outdated,
    /// Schema or workspace identity mismatch makes the artifact unsafe to trust.
    Incompatible,
}

/// Closed range of snapshot schema versions understood by one peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VersionSupport {
    /// Newest snapshot schema the peer can emit.
    pub current_schema_version: SchemaVersion,
    /// Oldest snapshot schema the peer promises to read.
    pub minimum_schema_version: SchemaVersion,
}

impl VersionSupport {
    /// Creates a version support range.
    pub const fn new(
        current_schema_version: SchemaVersion,
        minimum_schema_version: SchemaVersion,
    ) -> Self {
        Self {
            current_schema_version,
            minimum_schema_version,
        }
    }

    /// Support range compiled into the current build.
    pub const fn current_build() -> Self {
        Self::new(CURRENT_SCHEMA_VERSION, MINIMUM_READABLE_SCHEMA_VERSION)
    }
}

impl Default for VersionSupport {
    fn default() -> Self {
        Self::current_build()
    }
}

/// Request for the current snapshot status using the caller's readable range.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotStatusRequest {
    pub client_support: VersionSupport,
}

impl SnapshotStatusRequest {
    /// Creates a request using the caller's advertised support window.
    pub const fn new(client_support: VersionSupport) -> Self {
        Self { client_support }
    }
}

impl Default for SnapshotStatusRequest {
    fn default() -> Self {
        Self::new(VersionSupport::current_build())
    }
}

/// Explicit freshness status on the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum SnapshotFreshnessStatus {
    /// The peer has evaluated freshness against a concrete snapshot.
    Known(SnapshotFreshness),
    /// No snapshot was available to evaluate yet.
    Unknown,
}

/// Snapshot metadata needed by UIs and agents before downloading the artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotStatus {
    /// Schema version stored inside the snapshot artifact.
    pub schema_version: SchemaVersion,
    /// Compatibility of `schema_version` with the requesting peer.
    pub schema_compatibility: SchemaCompatibility,
    /// Freshness state reported by the producer.
    pub freshness: SnapshotFreshnessStatus,
}

/// Reply to a snapshot status request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SnapshotStatusResponse {
    /// A snapshot is available and its metadata could be evaluated.
    Available {
        #[serde(rename = "snapshot")]
        status: SnapshotStatus,
    },
    /// No snapshot exists yet for the requested workspace.
    Missing,
    /// The peer and snapshot do not share a readable schema window.
    VersionMismatch {
        server_support: VersionSupport,
        mismatch: VersionMismatchStatus,
    },
}

/// Request to negotiate a mutually readable snapshot schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotNegotiationRequest {
    pub client_support: VersionSupport,
}

impl SnapshotNegotiationRequest {
    /// Creates a schema negotiation request.
    pub const fn new(client_support: VersionSupport) -> Self {
        Self { client_support }
    }
}

impl Default for SnapshotNegotiationRequest {
    fn default() -> Self {
        Self::new(VersionSupport::current_build())
    }
}

/// Explicit reason no compatible schema could be negotiated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VersionMismatchStatus {
    /// The server only supports schemas older than the client can read.
    ServerTooOld,
    /// The client only supports schemas older than the server can read.
    ClientTooOld,
}

/// Result of snapshot schema negotiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SnapshotNegotiationResponse {
    /// A mutually readable schema version was found.
    Compatible { schema_version: SchemaVersion },
    /// No overlapping schema window exists.
    VersionMismatch {
        server_support: VersionSupport,
        mismatch: VersionMismatchStatus,
    },
}

impl SnapshotNegotiationResponse {
    /// Negotiates the newest schema version supported by both peers.
    pub const fn negotiate(
        request: SnapshotNegotiationRequest,
        server_support: VersionSupport,
    ) -> Self {
        if version_lt(
            server_support.current_schema_version,
            request.client_support.minimum_schema_version,
        ) {
            return Self::VersionMismatch {
                server_support,
                mismatch: VersionMismatchStatus::ServerTooOld,
            };
        }

        if version_lt(
            request.client_support.current_schema_version,
            server_support.minimum_schema_version,
        ) {
            return Self::VersionMismatch {
                server_support,
                mismatch: VersionMismatchStatus::ClientTooOld,
            };
        }

        Self::Compatible {
            schema_version: min_version(
                server_support.current_schema_version,
                request.client_support.current_schema_version,
            ),
        }
    }
}

const fn version_lt(lhs: SchemaVersion, rhs: SchemaVersion) -> bool {
    lhs.major < rhs.major || (lhs.major == rhs.major && lhs.minor < rhs.minor)
}

const fn min_version(lhs: SchemaVersion, rhs: SchemaVersion) -> SchemaVersion {
    if version_lt(lhs, rhs) { lhs } else { rhs }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        SchemaCompatibility, SchemaVersion, SnapshotFreshness,
        SnapshotFreshnessStatus, SnapshotNegotiationRequest,
        SnapshotNegotiationResponse, SnapshotStatus, SnapshotStatusRequest,
        SnapshotStatusResponse, VersionMismatchStatus, VersionSupport,
    };

    #[test]
    fn snapshot_status_response_serializes_stably() {
        let response = SnapshotStatusResponse::Available {
            status: SnapshotStatus {
                schema_version: SchemaVersion::new(1, 0),
                schema_compatibility: SchemaCompatibility::Current,
                freshness: SnapshotFreshnessStatus::Known(
                    SnapshotFreshness::Drifted,
                ),
            },
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["status"], json!("available"));
        assert_eq!(
            value["snapshot"]["schema_version"],
            json!({"major": 1, "minor": 0})
        );
        assert_eq!(value["snapshot"]["schema_compatibility"], json!("current"));
        assert_eq!(
            value["snapshot"]["freshness"],
            json!({
                "state": "known",
                "value": "drifted"
            })
        );
    }

    #[test]
    fn snapshot_status_response_version_mismatch_serializes_stably() {
        let response = SnapshotStatusResponse::VersionMismatch {
            server_support: VersionSupport::new(
                SchemaVersion::new(1, 1),
                SchemaVersion::new(1, 0),
            ),
            mismatch: VersionMismatchStatus::ClientTooOld,
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["status"], json!("version-mismatch"));
        assert_eq!(
            value["server_support"],
            json!({
                "current_schema_version": {"major": 1, "minor": 1},
                "minimum_schema_version": {"major": 1, "minor": 0}
            })
        );
        assert_eq!(value["mismatch"], json!("client-too-old"));
    }

    #[test]
    fn snapshot_status_request_ignores_unknown_future_fields() {
        let request: SnapshotStatusRequest = serde_json::from_value(json!({
            "client_support": {
                "current_schema_version": {"major": 1, "minor": 2},
                "minimum_schema_version": {"major": 1, "minor": 0}
            },
            "future_field": "ignored"
        }))
        .unwrap();

        assert_eq!(
            request.client_support,
            VersionSupport::new(SchemaVersion::new(1, 2), SchemaVersion::new(1, 0))
        );
    }

    #[test]
    fn snapshot_negotiation_response_ignores_unknown_variant_fields() {
        let response: SnapshotNegotiationResponse = serde_json::from_value(json!({
            "status": "version-mismatch",
            "server_support": {
                "current_schema_version": {"major": 1, "minor": 0},
                "minimum_schema_version": {"major": 1, "minor": 0}
            },
            "mismatch": "server-too-old",
            "extra_detail": "ignored"
        }))
        .unwrap();

        assert_eq!(
            response,
            SnapshotNegotiationResponse::VersionMismatch {
                server_support: VersionSupport::new(
                    SchemaVersion::new(1, 0),
                    SchemaVersion::new(1, 0),
                ),
                mismatch: VersionMismatchStatus::ServerTooOld,
            }
        );
    }

    #[test]
    fn snapshot_negotiation_reports_version_mismatch_explicitly() {
        let request = SnapshotNegotiationRequest {
            client_support: VersionSupport::new(
                SchemaVersion::new(1, 3),
                SchemaVersion::new(1, 2),
            ),
        };

        let response = SnapshotNegotiationResponse::negotiate(
            request,
            VersionSupport::new(SchemaVersion::new(1, 0), SchemaVersion::new(1, 0)),
        );

        assert_eq!(
            response,
            SnapshotNegotiationResponse::VersionMismatch {
                server_support: VersionSupport::new(
                    SchemaVersion::new(1, 0),
                    SchemaVersion::new(1, 0),
                ),
                mismatch: VersionMismatchStatus::ServerTooOld,
            }
        );
    }

    #[test]
    fn snapshot_negotiation_picks_newest_mutually_supported_schema() {
        let request = SnapshotNegotiationRequest {
            client_support: VersionSupport::new(
                SchemaVersion::new(1, 2),
                SchemaVersion::new(1, 0),
            ),
        };

        let response = SnapshotNegotiationResponse::negotiate(
            request,
            VersionSupport::new(SchemaVersion::new(1, 1), SchemaVersion::new(1, 0)),
        );

        assert_eq!(
            response,
            SnapshotNegotiationResponse::Compatible {
                schema_version: SchemaVersion::new(1, 1),
            }
        );
    }
}
