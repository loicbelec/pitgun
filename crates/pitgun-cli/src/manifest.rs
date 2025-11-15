use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub source: SourceConfig,
    pub processors: Vec<ProcessorConfig>,
    pub sink: SinkConfig,
}

#[derive(Debug, Deserialize)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub bind_addr: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct ProcessorConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub channels: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SinkConfig {
    #[serde(rename = "type")]
    pub r#type: String,
}

pub fn load_manifest_from_path(path: &str) -> Result<Manifest, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let manifest = serde_yaml::from_str(&contents)?;
    Ok(manifest)
}
