//! SOURCE OF TRUTH KEYWORDS: trash, freedesktop trash spec, move to trash, trashinfo, Replace original
//! WHAT:  Moves a file to the user's home trash (`$XDG_DATA_HOME/Trash`) the way the
//!        freedesktop.org Trash spec describes: the file under `files/`, a `.trashinfo`
//!        under `info/` with its original path and deletion time, names made unique.
//! WHY:   Quick trim's "Replace" sends the original to the trash, never deletes it outright.
//!        GNOME Files, Dolphin and the other file managers show and restore these entries.
//!        Plain std code, so it is tested on any OS.
//! WHERE: `media::GstMedia::trash`.

use std::path::{Path, PathBuf};

/// Percent-encode a path for `Path=` (the spec uses URL escaping; `/` stays).
fn encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.bytes() {
        let keep = b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The home trash directory under `home` (or `$XDG_DATA_HOME` when set and absolute).
pub fn trash_dir(home: &Path, xdg_data_home: Option<&str>) -> PathBuf {
    xdg_data_home
        .filter(|s| s.starts_with('/'))
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("Trash")
}

/// Move `file` into `trash`; `deleted_at` is the local time as `YYYY-MM-DDThh:mm:ss`.
/// Returns where it went. Fails (without touching the file) if the trash is on another
/// file system: the spec wants a per-volume trash there, which this does not create.
pub fn move_to_trash(file: &Path, trash: &Path, deleted_at: &str) -> std::io::Result<PathBuf> {
    let file = std::path::absolute(file)?;
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::other("file name is not valid UTF-8"))?
        .to_string();
    let (files, info) = (trash.join("files"), trash.join("info"));
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    for n in 1..10_000 {
        let candidate = if n == 1 {
            name.clone()
        } else {
            format!("{stem}.{n}{ext}")
        };
        let info_path = info.join(format!("{candidate}.trashinfo"));
        // `create_new` reserves the name atomically, as the spec asks.
        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info_path);
        let mut f = match created {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        use std::io::Write;
        let body = format!(
            "[Trash Info]\nPath={}\nDeletionDate={deleted_at}\n",
            encode_path(&file.to_string_lossy())
        );
        if let Err(e) = f.write_all(body.as_bytes()) {
            let _ = std::fs::remove_file(&info_path);
            return Err(e);
        }
        let dest = files.join(&candidate);
        if let Err(e) = std::fs::rename(&file, &dest) {
            let _ = std::fs::remove_file(&info_path);
            return Err(e);
        }
        return Ok(dest);
    }
    Err(std::io::Error::other(
        "the trash has too many files of that name",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_move_with_a_trashinfo_and_unique_names() {
        let root = std::env::temp_dir().join(format!("cortex-trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let trash = trash_dir(&root, None);
        let src = root.join("my clip.mov");
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..2 {
            std::fs::write(&src, format!("v{i}")).unwrap();
            let dest = move_to_trash(&src, &trash, "2026-11-03T10:15:00").unwrap();
            assert!(!src.exists());
            let want = if i == 0 {
                "my clip.mov"
            } else {
                "my clip.2.mov"
            };
            assert_eq!(dest, trash.join("files").join(want));
            let info =
                std::fs::read_to_string(trash.join("info").join(format!("{want}.trashinfo")))
                    .unwrap();
            assert!(info.starts_with("[Trash Info]\nPath=/"));
            assert!(info.contains("my%20clip.mov\n"));
            assert!(info.ends_with("DeletionDate=2026-11-03T10:15:00\n"));
        }
        assert!(move_to_trash(&root.join("missing.mov"), &trash, "x").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn xdg_data_home_is_respected() {
        assert_eq!(
            trash_dir(Path::new("/home/u"), Some("/data")),
            Path::new("/data/Trash")
        );
        assert_eq!(
            trash_dir(Path::new("/home/u"), None),
            Path::new("/home/u/.local/share/Trash")
        );
    }
}
