//! Fixed, hash-pinned Pitgun WASM only. No path, code, URL, environment or
//! runtime choice is accepted from the HTTP request.
use pitgun_contract::Digest;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest,
    evidence::{RacingOutputV1, RacingRunEvidenceV1},
    service_control::{
        RacingServiceAuthorizationResponseV1, RacingServiceDecisionV1, empty_timeline,
        validate_service_start,
    },
};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum ServiceReplayError {
    Mismatch,
    Unavailable,
}
#[derive(Clone, Debug)]
pub struct WasmServiceReplay {
    pub node: PathBuf,
    pub script: PathBuf,
    pub wasm: PathBuf,
    pub bridge: PathBuf,
    pub runtime_digest: Digest,
    pub bridge_digest: Digest,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CompactEvidence {
    output: RacingOutputV1,
    telemetry_summary: pitgun_contract::TelemetrySummaryV1,
}
#[derive(serde::Deserialize)]
struct Reply {
    ok: bool,
    evidence: Option<CompactEvidence>,
    code: Option<String>,
}
impl WasmServiceReplay {
    pub fn validate(&self) -> Result<(), String> {
        if self.bridge.extension().and_then(|s| s.to_str()) != Some("mjs") {
            return Err("service replay bridge must be a standalone .mjs file".into());
        }
        for path in [&self.node, &self.script, &self.wasm, &self.bridge] {
            if !path.is_absolute() || !path.is_file() {
                return Err("service replay paths must identify existing absolute files".into());
            }
        }
        if Digest::from_bytes(&fs::read(&self.wasm).map_err(|e| e.to_string())?)
            != self.runtime_digest
            || Digest::from_bytes(&fs::read(&self.bridge).map_err(|e| e.to_string())?)
                != self.bridge_digest
        {
            return Err("service replay bytes differ from their pinned identities".into());
        }
        Ok(())
    }
    pub fn verifier_identity(
        &self,
        base: &pitgun_contract::ArtifactIdentity,
    ) -> Result<pitgun_contract::ArtifactIdentity, String> {
        let mut identity = base.clone();
        identity.digest=pitgun_contract::canonical_json_digest(&serde_json::json!({"adapter":"pitgun.service-wasm-verifier/v1",
            "native_verifier":base,"node":Digest::from_bytes(&fs::read(&self.node).map_err(|e|e.to_string())?),
            "script":Digest::from_bytes(&fs::read(&self.script).map_err(|e|e.to_string())?),
            "wasm":self.runtime_digest,"bridge":self.bridge_digest})).map_err(|e|e.to_string())?;
        Ok(identity)
    }
    pub fn replay(
        &self,
        response: &RacingServiceAuthorizationResponseV1,
        catalog: &RacingCatalogSnapshot,
        history: &[RacingServiceDecisionV1],
    ) -> Result<RacingRunEvidenceV1, ServiceReplayError> {
        let config =
            validate_service_start(response, catalog).map_err(|_| ServiceReplayError::Mismatch)?;
        let input = response.canonical_input.clone();
        let request = RunRaceRequest {
            era: Some(input.era),
            hz: Some(input.hz),
            input,
            seed: response
                .signed
                .authorization
                .initial_contract
                .random
                .seed
                .get(),
        };
        let payload=serde_json::to_vec(&serde_json::json!({"requestJson":serde_json::to_string(&request).map_err(|_|ServiceReplayError::Unavailable)?,
            "catalogBundleJson":serde_json::to_string(&catalog.to_bundle().map_err(|_|ServiceReplayError::Unavailable)?).map_err(|_|ServiceReplayError::Unavailable)?,
            "timelineJson":serde_json::to_string(&empty_timeline()).map_err(|_|ServiceReplayError::Unavailable)?,
            "configJson":serde_json::to_string(&config).map_err(|_|ServiceReplayError::Unavailable)?,"history":history})).map_err(|_|ServiceReplayError::Unavailable)?;
        if payload.len() > 32 * 1024 * 1024 {
            return Err(ServiceReplayError::Unavailable);
        }
        let mut command = Command::new(&self.node);
        command
            .env_clear()
            .arg("--experimental-permission")
            .arg("--max-old-space-size=512");
        for path in [&self.script, &self.wasm, &self.bridge] {
            command.arg(format!("--allow-fs-read={}", path.display()));
        }
        let mut child = command
            .arg(&self.script)
            .arg(&self.wasm)
            .arg(&self.bridge)
            .arg(self.runtime_digest.to_string())
            .arg(self.bridge_digest.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ServiceReplayError::Unavailable)?;
        let mut stdin = child.stdin.take().ok_or(ServiceReplayError::Unavailable)?;
        let mut stdout = child.stdout.take().ok_or(ServiceReplayError::Unavailable)?;
        let writer = std::thread::spawn(move || stdin.write_all(&payload));
        let mut reader = Some(std::thread::spawn(move || {
            let mut bytes = vec![];
            stdout
                .by_ref()
                .take(65537)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        }));
        let started = Instant::now();
        let mut work = || -> Result<_, ServiceReplayError> {
            loop {
                match child
                    .try_wait()
                    .map_err(|_| ServiceReplayError::Unavailable)?
                {
                    Some(status) => {
                        if !status.success() {
                            return Err(ServiceReplayError::Unavailable);
                        }
                        break;
                    }
                    None if started.elapsed() > Duration::from_secs(120) => {
                        return Err(ServiceReplayError::Unavailable);
                    }
                    None => std::thread::sleep(Duration::from_millis(10)),
                }
            }
            let bytes = reader
                .take()
                .ok_or(ServiceReplayError::Unavailable)?
                .join()
                .map_err(|_| ServiceReplayError::Unavailable)?
                .map_err(|_| ServiceReplayError::Unavailable)?;
            if bytes.len() > 65536 {
                return Err(ServiceReplayError::Unavailable);
            }
            let reply: Reply =
                serde_json::from_slice(&bytes).map_err(|_| ServiceReplayError::Unavailable)?;
            if !reply.ok {
                return Err(if reply.code.as_deref() == Some("replay_mismatch") {
                    ServiceReplayError::Mismatch
                } else {
                    ServiceReplayError::Unavailable
                });
            }
            let e = reply.evidence.ok_or(ServiceReplayError::Unavailable)?;
            Ok(RacingRunEvidenceV1 {
                output: e.output,
                telemetry_summary: e.telemetry_summary,
            })
        };
        let result = work();
        // Always reap, including timeout, broken pipe, cancellation/error paths.
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
        if let Some(reader) = reader {
            let _ = reader.join();
        }
        let sent = writer.join();
        if result.is_ok() && !matches!(sent, Ok(Ok(()))) {
            return Err(ServiceReplayError::Unavailable);
        }
        result
    }
}
