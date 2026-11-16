//! Operator configuration, never a request-supplied runtime or filesystem path.
use pitgun_contract::Digest;
use serde::Deserialize;
use std::{fs::File, io::Read, path::PathBuf};

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_RETAINED_REPLAYS: usize = 8;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedReplays {
    pub schema: ManifestVersion,
    pub replays: Vec<ReplayConfig>,
}

#[derive(Deserialize)]
pub enum ManifestVersion {
    #[serde(rename = "pitgun.retained-service-replays/v1")]
    V1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayConfig {
    pub catalog_release_dir: PathBuf,
    pub node: PathBuf,
    pub script: PathBuf,
    pub wasm: PathBuf,
    pub bridge: PathBuf,
    pub runtime_digest: Digest,
    pub bridge_digest: Digest,
    pub timeout_seconds: u64,
}

impl RetainedReplays {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("retained service replay manifest must be an absolute path".into());
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::parse(&bytes)
    }

    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err("retained service replay manifest exceeds 64 KiB".into());
        }
        let manifest: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let ManifestVersion::V1 = manifest.schema;
        if manifest.replays.len() > MAX_RETAINED_REPLAYS {
            return Err("at most eight retained service replays may be configured".into());
        }
        for config in &manifest.replays {
            if !config.catalog_release_dir.is_absolute() || !config.catalog_release_dir.is_dir() {
                return Err(
                    "retained service catalog must be an existing absolute directory".into(),
                );
            }
            // The same bound as the primary replay; no request can override it.
            pitgun_verifier::wasm_service_replay::replay_timeout(Some(
                &config.timeout_seconds.to_string(),
            ))?;
        }
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_versioned_bounded_and_rejects_unknown_fields() {
        let valid = serde_json::json!({"schema":"pitgun.retained-service-replays/v1","replays":[]});
        assert!(RetainedReplays::parse(&serde_json::to_vec(&valid).unwrap()).is_ok());
        let mut wrong = valid.clone();
        wrong["schema"] = serde_json::json!("future");
        assert!(RetainedReplays::parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let mut wrong = valid.clone();
        wrong["url"] = serde_json::json!("https://untrusted.invalid/runtime");
        assert!(RetainedReplays::parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        assert!(RetainedReplays::parse(&vec![b' '; MAX_MANIFEST_BYTES as usize + 1]).is_err());
        let replay = serde_json::json!({
            "catalog_release_dir":PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.8.1"),
            "node":"/operator/node", "script":"/operator/replay.mjs", "wasm":"/operator/engine.wasm",
            "bridge":"/operator/bridge.mjs", "runtime_digest":Digest::from_bytes(b"runtime"),
            "bridge_digest":Digest::from_bytes(b"bridge"), "timeout_seconds":120,
        });
        let mut configured = valid.clone();
        configured["replays"] = serde_json::json!([replay.clone()]);
        assert!(RetainedReplays::parse(&serde_json::to_vec(&configured).unwrap()).is_ok());
        let mut wrong = configured.clone();
        wrong["replays"][0]["timeout_seconds"] = serde_json::json!(601);
        assert!(RetainedReplays::parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let mut wrong = configured.clone();
        wrong["replays"][0]["url"] = serde_json::json!("https://untrusted.invalid");
        assert!(RetainedReplays::parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let mut wrong = configured;
        wrong["replays"][0]["catalog_release_dir"] = serde_json::json!("relative/catalog");
        assert!(RetainedReplays::parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let mut wrong = valid;
        wrong["replays"] = serde_json::json!(vec![replay; 9]);
        assert!(
            RetainedReplays::parse(&serde_json::to_vec(&wrong).unwrap())
                .err()
                .unwrap()
                .contains("at most eight")
        );
    }
}
