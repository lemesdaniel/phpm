use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ComposerLock {
    #[serde(rename = "content-hash", default)]
    pub content_hash: String,
    #[serde(default)]
    pub packages: Vec<LockedPackage>,
    #[serde(rename = "packages-dev", default)]
    pub packages_dev: Vec<LockedPackage>,
    #[serde(rename = "plugin-api-version", default)]
    pub plugin_api_version: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    #[serde(rename = "type", default = "default_type")]
    pub package_type: String,
    pub dist: Option<Dist>,
    pub source: Option<Source>,
}

fn default_type() -> String {
    "library".to_string()
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Dist {
    #[serde(rename = "type")]
    pub dist_type: String,
    pub url: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub shasum: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Source {
    #[serde(rename = "type")]
    pub source_type: String,
    pub url: String,
    #[serde(default)]
    pub reference: String,
}
