use serde::Deserialize;
use serde::de;

#[derive(Debug, Clone)]
pub struct ConfigSource {
    pub urls: Vec<String>,
    pub name: String,
}

impl<'de> Deserialize<'de> for ConfigSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSource {
            #[serde(default)]
            name: String,
            url: Option<serde_json::Value>,
            #[serde(default)]
            urls: Vec<String>,
        }

        let raw = RawSource::deserialize(deserializer)?;
        let urls = if !raw.urls.is_empty() {
            raw.urls
        } else if let Some(val) = raw.url {
            match val {
                serde_json::Value::String(s) => vec![s],
                serde_json::Value::Array(arr) => {
                    arr.into_iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }
                _ => return Err(de::Error::custom("url must be a string or array of strings")),
            }
        } else {
            return Err(de::Error::custom("missing url or urls field"));
        };

        Ok(ConfigSource { urls, name: raw.name })
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_max_volume")]
    pub max_volume: i32,
    #[serde(default = "default_volume_step")]
    pub volume_step: i32,
    #[serde(default)]
    pub sources: Vec<ConfigSource>,
}

fn default_max_volume() -> i32 { 10 }
fn default_volume_step() -> i32 { 1 }

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
}
