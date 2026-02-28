use crate::data::DataRegistry;
use crate::provider::InMemoryConfigProvider;

pub fn default_in_memory_provider() -> InMemoryConfigProvider {
    DataRegistry::load_default()
        .expect("embedded simulator data pack must be valid")
        .into_provider()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ConfigProvider;

    #[test]
    fn defaults_are_resolvable() {
        let provider = default_in_memory_provider();
        let track = provider.get_track("SPA").expect("spa track");
        let vehicle = provider.get_vehicle("f1_2026").expect("f1 vehicle");
        assert!(track.s_m.len() > 100);
        assert_eq!(vehicle.engine_id, "v6t_hybrid");
        assert_eq!(track.pit_loss_ms, 22_000);
    }
}
