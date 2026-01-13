use std::fs;
use pitgun_contract::SignedSimulationContractV1;
use pitgun_signing::SigningKey;

fn main() {
    let path = std::env::args().nth(1).expect("usage: verify_sim <contract.json>");
    let raw = fs::read_to_string(&path).expect("read json");

    let signed: SignedSimulationContractV1 =
        serde_json::from_str(&raw).expect("parse signed contract");

    let bytes = signed.contract.signing_bytes().expect("signing bytes");
    let key = SigningKey::from_env().expect("PITGUN_SIGNING_SECRET");

    println!("signature_ok={}", key.verify(&bytes, &signed.signature));
}
