use serde::Deserialize;
use std::collections::BTreeMap;

/// String OR array of strings — normalizes to Vec<String>.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
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
    #[serde(default, deserialize_with = "scripts_map")]
    pub scripts: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "string_or_vec")]
    pub bin: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Autoload {
    #[serde(rename = "psr-4", default, deserialize_with = "psr_map")]
    pub psr4: BTreeMap<String, Vec<String>>,
    #[serde(rename = "psr-0", default, deserialize_with = "psr_map")]
    pub psr0: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub classmap: Vec<String>,
}

/// `bin` pode ser string única ou lista → normaliza p/ Vec.
fn string_or_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(OneOrMany::deserialize(d)?.into_vec())
}

/// psr-4 / psr-0: cada valor é string OU lista de strings → normaliza p/ Vec.
fn psr_map<'de, D>(d: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: BTreeMap<String, OneOrMany> = BTreeMap::deserialize(d)?;
    Ok(raw.into_iter().map(|(k, v)| (k, v.into_vec())).collect())
}

/// scripts: cada valor é string única OU lista → normaliza p/ Vec.
fn scripts_map<'de, D>(d: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: BTreeMap<String, OneOrMany> = BTreeMap::deserialize(d)?;
    Ok(raw.into_iter().map(|(k, v)| (k, v.into_vec())).collect())
}
