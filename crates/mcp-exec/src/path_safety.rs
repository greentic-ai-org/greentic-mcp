use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Normalize a user-supplied path and ensure it stays within an allowed root.
/// Reject absolute paths and any that escape via `..`.
pub fn normalize_under_root(root: &Path, candidate: &Path) -> Result<PathBuf> {
    if candidate.is_absolute() {
        anyhow::bail!("absolute paths are not allowed: {}", candidate.display());
    }

    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;

    let joined = root.join(candidate);
    let canon = match joined.canonicalize() {
        Ok(path) => path,
        Err(_err) if !joined.exists() => {
            let parent = joined
                .parent()
                .context("path has no parent to normalize")?
                .canonicalize()
                .with_context(|| format!("failed to canonicalize {}", joined.display()))?;

            parent.join(joined.file_name().context("path missing final component")?)
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to canonicalize {}", joined.display()));
        }
    };

    if !canon.starts_with(&root) {
        anyhow::bail!(
            "path escapes root ({}): {}",
            root.display(),
            canon.display()
        );
    }

    Ok(canon)
}
