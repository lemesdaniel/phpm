use serde::Deserialize;
use std::collections::BTreeMap;

/// String OR array-of-strings OR anything else — normalizes to Vec<String>.
/// The "anything else" arm silently discards object-valued script entries like
/// Symfony Flex's `"auto-scripts": {"cache:clear": "symfony-cmd", ...}`.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
    Other(#[allow(dead_code)] serde_json::Value),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
            OneOrMany::Other(_) => vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ComposerJson {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub require: BTreeMap<String, String>,
    #[serde(rename = "require-dev", default)]
    pub require_dev: BTreeMap<String, String>,
    #[serde(default)]
    pub autoload: Autoload,
    #[serde(rename = "autoload-dev", default)]
    pub autoload_dev: Autoload,
    #[serde(default, deserialize_with = "one_or_many_map")]
    pub scripts: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "string_or_vec")]
    pub bin: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Autoload {
    #[serde(rename = "psr-4", default, deserialize_with = "one_or_many_map")]
    pub psr4: BTreeMap<String, Vec<String>>,
    #[serde(rename = "psr-0", default, deserialize_with = "one_or_many_map")]
    pub psr0: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub classmap: Vec<String>,
}

/// `bin` can be a single string or a list → normalizes to Vec.
/// Tolerates explicit `null` (becomes an empty Vec).
fn string_or_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<OneOrMany> = Option::deserialize(d)?;
    Ok(opt.map(OneOrMany::into_vec).unwrap_or_default())
}

/// psr-4 / psr-0 / scripts: each value is a string OR a list → normalizes to Vec.
/// Tolerates explicit `null` on the map (becomes an empty BTreeMap).
fn one_or_many_map<'de, D>(d: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<BTreeMap<String, OneOrMany>> = Option::deserialize(d)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, v.into_vec()))
        .collect())
}
