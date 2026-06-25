use lockfile::ComposerJson;
use std::collections::BTreeMap;

/// Which PHP base variable a path is anchored to in the generated files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathBase {
    /// `$baseDir` — the project root (root package's own autoload).
    Base,
    /// `$vendorDir` — the vendor directory (dependency packages).
    Vendor,
}

impl PathBase {
    /// A base-tagged path, e.g. "$vendorDir/monolog/monolog/src". The emitter later turns
    /// the tag into PHP variable concatenation; a bare "" yields just the variable.
    pub fn join(self, rel: &str) -> String {
        let var = match self {
            PathBase::Base => "$baseDir",
            PathBase::Vendor => "$vendorDir",
        };
        let rel = rel.trim_start_matches('/').trim_end_matches('/');
        if rel.is_empty() {
            var.to_string()
        } else {
            format!("{var}/{rel}")
        }
    }
}

/// Aggregated autoload rules across all packages, ready to emit.
#[derive(Debug, Default)]
pub struct AutoloadData {
    pub psr4: BTreeMap<String, Vec<String>>,
    pub psr0: BTreeMap<String, Vec<String>>,
    pub files: Vec<String>,
    pub classmap_dirs: Vec<String>,
}

/// Merge one package's autoload block into `data`. `pkg_prefix` is the vendor-relative
/// "vendor/package" segment for dependencies, or None for the root package.
pub fn aggregate_autoload(
    data: &mut AutoloadData,
    json: &ComposerJson,
    base: PathBase,
    pkg_prefix: Option<&str>,
) {
    let prefixed = |rel: &str| -> String {
        match pkg_prefix {
            Some(p) => format!("{}/{}", p.trim_end_matches('/'), rel.trim_start_matches('/')),
            None => rel.to_string(),
        }
    };

    for (ns, dirs) in &json.autoload.psr4 {
        let entry = data.psr4.entry(ns.clone()).or_default();
        for d in dirs {
            entry.push(base.join(&prefixed(d)));
        }
    }
    for (ns, dirs) in &json.autoload.psr0 {
        let entry = data.psr0.entry(ns.clone()).or_default();
        for d in dirs {
            entry.push(base.join(&prefixed(d)));
        }
    }
    for f in &json.autoload.files {
        data.files.push(base.join(&prefixed(f)));
    }
    for c in &json.autoload.classmap {
        data.classmap_dirs.push(base.join(&prefixed(c)));
    }
}
