use sha2::{Digest, Sha256};
use std::io;
use std::path::Path;
use walkdir::WalkDir;

/// Hash determinístico do conteúdo de um diretório.
/// Ordena os arquivos pelo caminho relativo (com `/` normalizado) e mistura,
/// para cada arquivo: caminho relativo, tamanho e bytes. Independente da
/// ordem de criação no FS e da plataforma (separador normalizado).
/// Assume nomes de arquivo UTF-8 (pacotes Composer usam ASCII na prática);
/// não aplica normalização Unicode NFC/NFD — fora do escopo M1.
pub fn sha256_tree(root: &Path) -> io::Result<String> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(io::Error::other)?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_str()
            .ok_or_else(|| io::Error::other("caminho não-UTF-8 no pacote"))?
            .replace('\\', "/");
        files.push((rel, entry.path().to_path_buf()));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel, abs) in files {
        let bytes = std::fs::read(&abs)?;
        hasher.update((rel.len() as u64).to_le_bytes());
        hasher.update(rel.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}
