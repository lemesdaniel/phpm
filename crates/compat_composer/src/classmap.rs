use crate::GenError;
use std::collections::BTreeMap;
use std::path::Path;
use store::{PackageCoords, Store};
use walkdir::WalkDir;

/// Scan a directory tree for PHP class/interface/trait/enum declarations.
/// Returns FQCN → file path relative to `root` (forward slashes). This is a lightweight
/// line tokenizer covering the declaration forms Composer's classmap generator handles;
/// PHP requires one namespace per file and top-level type declarations use fixed syntax,
/// so a full parser is unnecessary here.
///
/// Known limitation: multi-line `/* ... */` block comments are not tracked, so a
/// `class`/`interface`/`trait`/`enum` keyword appearing unindented on its own line
/// inside a block comment would produce a spurious entry. Rare in real packages;
/// accepted as a carry for this heuristic line-based tokenizer.
pub fn scan_php_classes(root: &Path) -> Result<BTreeMap<String, String>, GenError> {
    let mut out = BTreeMap::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| GenError::Io(std::io::Error::other(e)))?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("php") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| GenError::Io(std::io::Error::other(e)))?
            .to_string_lossy()
            .replace('\\', "/");
        // Skip files that aren't valid UTF-8 (binary assets, non-UTF-8 encodings).
        let bytes = std::fs::read(entry.path())?;
        let src = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for fqcn in classes_in_source(&src) {
            out.insert(fqcn, rel.clone());
        }
    }
    Ok(out)
}

fn classes_in_source(src: &str) -> Vec<String> {
    let mut namespace = String::new();
    let mut names = Vec::new();
    // Track heredoc/nowdoc state: Some(terminator) while inside a heredoc.
    let mut heredoc: Option<String> = None;
    for raw in src.lines() {
        // Heredoc exit: a line whose trimmed content is the terminator (with optional trailing `;`).
        // PHP 7.3+ allows the terminator to be indented.
        if let Some(ref term) = heredoc {
            let trimmed = raw.trim().trim_end_matches(';').trim_end();
            if trimmed == term {
                heredoc = None;
            }
            continue;
        }
        // Heredoc entry: `<<<EOT`, `<<<'EOT'`, `<<<"EOT"` — may appear anywhere on a line.
        if let Some(pos) = raw.find("<<<") {
            let after = raw[pos + 3..].trim_end();
            // Strip surrounding quotes for nowdoc / double-quoted heredoc.
            let ident = after
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('\'')
                .trim_end_matches('\'');
            if !ident.is_empty() && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                heredoc = Some(ident.to_string());
                continue;
            }
        }
        let line = strip_line_comment(raw).trim();
        if let Some(ns) = parse_namespace(line) {
            namespace = ns;
            continue;
        }
        if let Some(name) = parse_type_decl(line) {
            names.push(if namespace.is_empty() {
                name
            } else {
                format!("{namespace}\\{name}")
            });
        }
    }
    names
}

fn strip_line_comment(line: &str) -> &str {
    let cut = line
        .find("//")
        .or_else(|| line.find('#'))
        .unwrap_or(line.len());
    &line[..cut]
}

fn parse_namespace(line: &str) -> Option<String> {
    let rest = line.strip_prefix("namespace ")?;
    Some(
        rest.trim_end_matches(';')
            .trim()
            .trim_matches('{')
            .trim()
            .to_string(),
    )
}

fn parse_type_decl(line: &str) -> Option<String> {
    let mut words = line.split_whitespace();
    let mut kw = words.next()?;
    while matches!(kw, "final" | "abstract" | "readonly") {
        kw = words.next()?;
    }
    if !matches!(kw, "class" | "interface" | "trait" | "enum") {
        return None;
    }
    let name = words.next()?;
    let name = name.split(':').next().unwrap_or(name); // enum "Level:" / "Level: string"
    let first = name.chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    Some(name.to_string())
}

/// Classmap for a stored package, cached at `<store>/meta/<vendor>/<package>/<version>.classmap.json`.
/// Scans on a cache miss. Returns FQCN → "$vendorDir/<vendor>/<package>/<rel>" ready to emit.
pub fn classmap_for_package(
    store: &Store,
    coords: &PackageCoords,
    vendor_pkg_dir: &Path,
) -> Result<BTreeMap<String, String>, GenError> {
    let meta = store.meta_path(coords);
    // build the cache filename explicitly (NOT with_extension — versions have dots)
    let cache = meta
        .parent()
        .map(|p| p.join(format!("{}.classmap.json", coords.version)))
        .ok_or_else(|| GenError::Io(std::io::Error::other("meta path has no parent")))?;

    if let Ok(raw) = std::fs::read_to_string(&cache) {
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(&raw) {
            return Ok(rebase(map, coords));
        }
    }

    let scanned = scan_php_classes(vendor_pkg_dir)?;
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::write(&cache, serde_json::to_vec(&scanned)?); // cache is best-effort
    Ok(rebase(scanned, coords))
}

fn rebase(map: BTreeMap<String, String>, coords: &PackageCoords) -> BTreeMap<String, String> {
    let prefix = format!("$vendorDir/{}/{}", coords.vendor, coords.package);
    map.into_iter()
        .map(|(class, rel)| (class, format!("{prefix}/{rel}")))
        .collect()
}
