//! Validate all template media destinations before extracting any files.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::{EngineError, Result, Session};

fn relative(name: &str) -> std::result::Result<Option<&str>, String> {
    let Some(rel) = name.strip_prefix("media/") else { return Ok(None) };
    for part in rel.split('/') {
        let stem = part.split('.').next().unwrap_or_default().to_ascii_uppercase();
        let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
            || ["COM", "LPT"]
                .iter()
                .any(|p| stem.strip_prefix(*p).is_some_and(|s| matches!(s, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")));
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with(['.', ' '])
            || part.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
            || part.chars().any(char::is_control)
            || device
        {
            return Err(format!("unsafe template media path: {name}"));
        }
    }
    Ok(Some(rel))
}

pub(crate) fn validate(entries: &[(String, Vec<u8>)]) -> std::result::Result<(), String> {
    let mut names = HashSet::new();
    for (name, _) in entries {
        relative(name)?;
        // Case variants address the same file on common Windows/macOS file systems.
        if !names.insert(name.to_lowercase()) {
            return Err(format!("duplicate template entry: {name}"));
        }
    }
    Ok(())
}

fn check_destination(root: &Path, output: &Path) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    for path in output.ancestors() {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(std::io::Error::other(format!("template destination is a symbolic link: {}", path.display())));
            }
            Ok(_) if path == output => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("template destination already exists: {} (choose a fresh footage folder)", path.display()),
                ));
            }
            Ok(meta) if path != output && !meta.is_dir() => {
                return Err(std::io::Error::other(format!("template directory is not a folder: {}", path.display())));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        // Ancestors above the explicitly chosen extraction root are the user's path.
        if path == root {
            break;
        }
    }
    let _ = (root, output);
    Ok(())
}

pub(crate) fn extract(s: &Session, entries: &[(String, Vec<u8>)], dir: &str, flatten: bool) -> Result<HashMap<String, String>> {
    validate(entries).map_err(EngineError::Other)?;
    let root = Path::new(dir);
    let mut used = HashSet::new();
    let mut planned = vec![];
    for (name, bytes) in entries {
        let Some(rel) = relative(name).map_err(EngineError::Other)? else { continue };
        let file = rel.rsplit('/').next().unwrap_or(rel);
        let mut destination = if flatten { file.to_string() } else { rel.to_string() };
        let mut suffix = 1;
        while !used.insert(destination.to_lowercase()) {
            destination = format!("{suffix}-{file}");
            suffix += 1;
        }
        // '/' is also accepted by native Windows paths, and keeps browser storage keys portable.
        let output = format!("{dir}/{destination}");
        check_destination(root, Path::new(&output)).map_err(|e| EngineError::Other(e.to_string()))?;
        planned.push((name, bytes, output));
    }
    let destinations: HashSet<String> = planned.iter().map(|(_, _, output)| output.to_lowercase()).collect();
    for (_, _, output) in &planned {
        if Path::new(output).ancestors().skip(1).any(|p| destinations.contains(&p.to_string_lossy().to_lowercase())) {
            return Err(EngineError::Other("template media entries conflict as files and directories".into()));
        }
    }
    let mut written = HashMap::new();
    for (name, bytes, output) in planned {
        s.services.store_file(&output, bytes).map_err(|e| EngineError::Other(format!("cannot write {output}: {e}")))?;
        if let Some(importer) = &s.importer {
            importer.register(&output, bytes);
        }
        written.insert(name.clone(), output);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!("effectcraft-template-audit-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn unsafe_archive_paths_and_duplicates_fail_before_any_extraction() {
        let root = temp();
        let outside = root.join("outside.txt");
        std::fs::write(&outside, b"keep").unwrap();
        let mut session = Session::default();
        let cid = session.execute("comp.new", json!({"width": 8, "height": 8})).unwrap()["comp"].clone();
        let original = session.project.clone();
        for bad in [
            "media/../outside.txt".to_string(),
            "media/../../outside.txt".into(),
            format!("media/{}", outside.display()),
            "media/\\absolute.txt".into(),
            "media/C:/outside.txt".into(),
            "media/..\\outside.txt".into(),
            "media/good.txt".into(),
            "media/GOOD.TXT".into(),
            "media/NUL.txt".into(),
            "media/CONOUT$".into(),
            "media/COM¹.txt".into(),
            "media/bad?.txt".into(),
        ] {
            let entries = vec![
                (
                    "manifest.json".into(),
                    serde_json::to_vec(&json!({"format": crate::commands::essential::TEMPLATE_FORMAT, "version": 1, "comp": cid})).unwrap(),
                ),
                ("project.ecproj".into(), session.project.to_json().into_bytes()),
                ("media/good.txt".into(), b"normal".to_vec()),
                (bad, b"overwrite".to_vec()),
            ];
            let archive = root.join("bad.ectemplate");
            std::fs::write(&archive, effectcraft_lottie::zip::store(&entries)).unwrap();
            assert!(session.execute("essential.importTemplate", json!({"path": archive, "addToComp": false})).is_err());
            assert_eq!(*session.project, *original);
            assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
            assert!(!root.join("bad Media").exists());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn destination_conflicts_fail_before_writing_earlier_entries() {
        let root = temp();
        let entries = vec![("media/good.txt".into(), vec![1]), ("media/a".into(), vec![2]), ("media/a/b".into(), vec![3])];
        assert!(extract(&Session::default(), &entries, root.to_str().unwrap(), false).is_err());
        assert!(!root.join("good.txt").exists());
        std::fs::create_dir(root.join("a")).unwrap();
        assert!(extract(&Session::default(), &entries[..2], root.to_str().unwrap(), false).is_err());
        assert!(!root.join("good.txt").exists());
        std::fs::write(root.join("keep.txt"), b"keep").unwrap();
        let existing = vec![("media/good.txt".into(), vec![1]), ("media/keep.txt".into(), vec![2])];
        assert!(extract(&Session::default(), &existing, root.to_str().unwrap(), false).is_err());
        assert!(!root.join("good.txt").exists());
        assert_eq!(std::fs::read(root.join("keep.txt")).unwrap(), b"keep");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_media_destinations_are_rejected_before_any_writes() {
        use std::os::unix::fs::symlink;
        let root = temp();
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("keep.txt"), b"keep").unwrap();
        let output = root.join("extracted");
        std::fs::create_dir(&output).unwrap();
        symlink(&outside, output.join("linked")).unwrap();
        let entries = vec![("media/good.txt".into(), vec![1]), ("media/linked/keep.txt".into(), vec![2])];
        assert!(extract(&Session::default(), &entries, output.to_str().unwrap(), false).is_err());
        assert!(!output.join("good.txt").exists());
        assert_eq!(std::fs::read(outside.join("keep.txt")).unwrap(), b"keep");
        symlink(&outside, root.join("linked-root")).unwrap();
        assert!(extract(&Session::default(), &entries[..1], root.join("linked-root").to_str().unwrap(), true).is_err());
        assert!(!outside.join("good.txt").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
