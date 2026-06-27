use lockfile::{Autoload, ComposerJson};
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
    /// Declared classmap dirs from composer.json. NOTE: generate() currently builds the
    /// classmap by full-walking each package (classmap.rs), so this is collected but not yet
    /// consulted — a future precise pass could honor only the declared dirs. TODO.
    pub classmap_dirs: Vec<String>,
}

/// Merge a package's `autoload` block into `data`. `pkg_prefix` is the vendor-relative
/// "vendor/package" segment for dependencies, or None for the root package.
pub fn aggregate_autoload(
    data: &mut AutoloadData,
    json: &ComposerJson,
    base: PathBase,
    pkg_prefix: Option<&str>,
) {
    merge_block(data, &json.autoload, base, pkg_prefix);
}

/// Merge a package's `autoload-dev` block into `data`. Composer only applies autoload-dev for
/// the root package (and only outside `--no-dev`), so generate() calls this for the root alone.
pub fn aggregate_autoload_dev(
    data: &mut AutoloadData,
    json: &ComposerJson,
    base: PathBase,
    pkg_prefix: Option<&str>,
) {
    merge_block(data, &json.autoload_dev, base, pkg_prefix);
}

fn merge_block(
    data: &mut AutoloadData,
    autoload: &Autoload,
    base: PathBase,
    pkg_prefix: Option<&str>,
) {
    let prefixed = |rel: &str| -> String {
        match pkg_prefix {
            Some(p) => format!(
                "{}/{}",
                p.trim_end_matches('/'),
                rel.trim_start_matches('/')
            ),
            None => rel.to_string(),
        }
    };

    for (ns, dirs) in &autoload.psr4 {
        let entry = data.psr4.entry(ns.clone()).or_default();
        for d in dirs {
            entry.push(base.join(&prefixed(d)));
        }
    }
    for (ns, dirs) in &autoload.psr0 {
        let entry = data.psr0.entry(ns.clone()).or_default();
        for d in dirs {
            entry.push(base.join(&prefixed(d)));
        }
    }
    for f in &autoload.files {
        data.files.push(base.join(&prefixed(f)));
    }
    for c in &autoload.classmap {
        data.classmap_dirs.push(base.join(&prefixed(c)));
    }
}
