use pitgun_contract::game::v1::GameSimulationContractV1;
use pitgun_signing::{SigningKey, verify_game_contract_v1_with_key};
use std::fs;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: verify_sim <signed_request.json>");
    let raw = fs::read_to_string(&path).expect("read json");

    let contract: GameSimulationContractV1 = serde_json::from_str(&raw).expect("parse contract");
    let key = SigningKey::from_env().expect("PITGUN_SIGNING_SECRET");
    let result = verify_game_contract_v1_with_key(&contract, &key).expect("verify");

    println!("signature_ok={}", result.signature_ok);
    println!("contract_expired={}", result.expired);
}
