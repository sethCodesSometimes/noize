use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceState {
    pub url: String,
    pub volume: i32,
    pub muted: bool,
    pub previous_volume: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    pub current_index: usize,
    pub sources: Vec<SourceState>,
}

impl State {
    pub fn load(path: &str) -> anyhow::Result<Option<Self>> {
        if !Path::new(path).exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        let state: State = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
