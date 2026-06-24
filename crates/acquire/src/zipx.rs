use crate::AcquireError;
use std::io::{Cursor, Read};
use std::path::Path;

/// Extrai um zip (bytes) para `dest`, removendo o único diretório-raiz que os
/// archives do Composer/Packagist embrulham (ex.: `vendor-pkg-<hash>/`).
/// Se os arquivos NÃO compartilham um único dir-raiz, extrai sem strip.
pub fn extract_strip_root(zip_bytes: &[u8], dest: &Path) -> Result<(), AcquireError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| AcquireError::Zip(e.to_string()))?;

    let root = common_root(&mut archive)?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AcquireError::Zip(e.to_string()))?;
        let name = file.name().to_string();
        let rel = match &root {
            Some(prefix) => name.strip_prefix(prefix.as_str()).unwrap_or(&name),
            None => name.as_str(),
        };
        if rel.is_empty() {
            continue; // a própria entrada do dir-raiz
        }
        if rel.contains("..") {
            return Err(AcquireError::Zip(format!("entry suspeita: {name}")));
        }
        if rel.starts_with('/') {
            return Err(AcquireError::Zip(format!("entry com caminho absoluto: {name}")));
        }
        let out = dest.join(rel);
        if file.is_dir() || name.ends_with('/') {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut bytes = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut bytes)
                .map_err(|e| AcquireError::Zip(e.to_string()))?;
            std::fs::write(&out, bytes)?;
        }
    }
    Ok(())
}

/// Retorna `Some("<root>/")` se TODAS as entradas começam com o mesmo primeiro
/// componente; senão `None` (não faz strip).
fn common_root<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Option<String>, AcquireError> {
    let mut root: Option<String> = None;
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| AcquireError::Zip(e.to_string()))?;
        let name = file.name();
        let first = match name.split_once('/') {
            Some((head, _)) => head.to_string(),
            None => return Ok(None),
        };
        match &root {
            None => root = Some(first),
            Some(r) if *r != first => return Ok(None),
            _ => {}
        }
    }
    Ok(root.map(|r| format!("{r}/")))
}
