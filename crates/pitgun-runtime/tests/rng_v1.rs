use pitgun_contract::Seed;
use pitgun_runtime::rng::{SplitMix64V1, derive_stream_seed_v1};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn splitmix64_v1_vectors_match_across_runtimes() {
    let mut zero = SplitMix64V1::from_u64(0);
    let mut maximum = SplitMix64V1::from_u64(u64::MAX);

    assert_eq!(
        [zero.next_u64(), zero.next_u64(), zero.next_u64()],
        [
            0xE220_A839_7B1D_CDAF,
            0x6E78_9E6A_A1B9_65F4,
            0x06C4_5D18_8009_454F,
        ]
    );
    assert_eq!(
        [maximum.next_u64(), maximum.next_u64()],
        [0xE4D9_7177_1B65_2C20, 0xE99F_F867_DBF6_82C9]
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn sha256_label_v1_vectors_match_across_runtimes() {
    let actual = [
        derive_stream_seed_v1(Seed::new(0), "solver", "entity-0", 0)
            .unwrap()
            .get(),
        derive_stream_seed_v1(Seed::new(7), "racing.lap", "player", 1)
            .unwrap()
            .get(),
        derive_stream_seed_v1(
            Seed::new(u64::MAX),
            "grid.node",
            "poste-électrique",
            u64::MAX,
        )
        .unwrap()
        .get(),
    ];

    assert_eq!(
        actual,
        [
            0xD34D_C81F_E421_A5AD,
            0x29E0_A030_58DC_9787,
            0x34B6_B94D_D89B_109D,
        ]
    );
}
