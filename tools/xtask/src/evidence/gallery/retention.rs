//! Finite validation and indexed eviction for retained gallery history.

use std::{fs, path::Path};

use super::{validate_commit, write_gallery_index, GalleryIndex};

const MAX_GALLERY_FILES: usize = 1_024;
const MAX_GALLERY_BYTES: u64 = 600 * 1024 * 1024;

pub(super) fn validate_existing_tree(root: &Path, index: &GalleryIndex) -> Result<(), String> {
    for entry in
        fs::read_dir(root).map_err(|error| format!("cannot inspect existing gallery: {error}"))?
    {
        let entry = entry.map_err(|error| format!("cannot inspect gallery entry: {error}"))?;
        let name = entry.file_name();
        if !matches!(
            name.to_str(),
            Some(".nojekyll" | "index.html" | "gallery.json" | "current" | "commits")
        ) {
            return Err(format!(
                "existing gallery contains undeclared root entry {}",
                entry.path().display()
            ));
        }
    }
    let commits_root = root.join("commits");
    if commits_root.is_dir() {
        for entry in fs::read_dir(&commits_root)
            .map_err(|error| format!("cannot inspect gallery commits: {error}"))?
        {
            let entry = entry.map_err(|error| format!("cannot inspect gallery commit: {error}"))?;
            let commit = entry
                .file_name()
                .into_string()
                .map_err(|_| "gallery commit directory is not valid UTF-8")?;
            if !index.commits.contains(&commit) {
                return Err(format!("gallery contains unindexed commit '{commit}'"));
            }
        }
    }
    tree_bounds(root)?;
    Ok(())
}

pub(super) fn trim_indexed_history_to_bounds(
    root: &Path,
    index: &mut GalleryIndex,
    has_conduitos: bool,
) -> Result<(), String> {
    loop {
        let (files, bytes) = tree_bounds(root)?;
        if files <= MAX_GALLERY_FILES && bytes <= MAX_GALLERY_BYTES {
            return Ok(());
        }
        if index.commits.len() <= 1 {
            return Err("gallery exceeds its finite file or byte bound after retention".into());
        }
        let commit = index
            .commits
            .pop()
            .ok_or_else(|| "gallery retention index became empty".to_string())?;
        validate_commit(&commit)?;
        let path = root.join("commits").join(commit);
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("cannot evict bounded gallery history: {error}"))?;
        }
        write_gallery_index(root, index, has_conduitos)?;
    }
}

fn tree_bounds(root: &Path) -> Result<(usize, u64), String> {
    let mut files = 0_usize;
    let mut bytes = 0_u64;
    accumulate_tree_bounds(root, &mut files, &mut bytes)?;
    Ok((files, bytes))
}

fn accumulate_tree_bounds(
    directory: &Path,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), String> {
    for entry in
        fs::read_dir(directory).map_err(|error| format!("cannot inspect gallery tree: {error}"))?
    {
        let entry = entry.map_err(|error| format!("cannot inspect gallery tree entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect gallery tree type: {error}"))?;
        if file_type.is_symlink() {
            return Err(format!(
                "gallery contains symlink {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            accumulate_tree_bounds(&entry.path(), files, bytes)?;
        } else if file_type.is_file() {
            *files = files
                .checked_add(1)
                .ok_or_else(|| "gallery file count overflow".to_string())?;
            *bytes = bytes
                .checked_add(
                    entry
                        .metadata()
                        .map_err(|error| format!("cannot inspect gallery file: {error}"))?
                        .len(),
                )
                .ok_or_else(|| "gallery byte count overflow".to_string())?;
        } else {
            return Err(format!(
                "gallery contains special entry {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::gallery::{GalleryIndex, GALLERY_SCHEMA, RETAINED_COMMITS};
    use std::path::PathBuf;

    fn temporary_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("conduit-gallery-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("commits")).unwrap();
        root
    }

    fn commit(sequence: usize) -> String {
        format!("{sequence:040x}")
    }

    #[test]
    fn full_valid_history_evicts_only_oldest_indexed_commits() {
        let root = temporary_root("bounded-retention");
        let commits = (0..RETAINED_COMMITS).rev().map(commit).collect::<Vec<_>>();
        for commit in &commits {
            let directory = root.join("commits").join(commit);
            fs::create_dir_all(&directory).unwrap();
            for file in 0..32 {
                fs::write(directory.join(format!("{file}.evidence")), []).unwrap();
            }
        }
        let oldest = commits.last().unwrap().clone();
        let newest = commits.first().unwrap().clone();
        let mut index = GalleryIndex {
            schema: GALLERY_SCHEMA.into(),
            current_commit: newest.clone(),
            retention_commits: RETAINED_COMMITS,
            commits,
        };
        write_gallery_index(&root, &index, false).unwrap();
        assert!(tree_bounds(&root).unwrap().0 > MAX_GALLERY_FILES);
        validate_existing_tree(&root, &index).unwrap();
        trim_indexed_history_to_bounds(&root, &mut index, false).unwrap();
        assert!(root.join("commits").join(newest).is_dir());
        assert!(!root.join("commits").join(oldest).exists());
        assert!(index.commits.len() < RETAINED_COMMITS);
        assert!(tree_bounds(&root).unwrap().0 <= MAX_GALLERY_FILES);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn hostile_tree_refuses_before_any_indexed_eviction() {
        use std::os::unix::fs::symlink;
        let root = temporary_root("hostile-retention");
        let exact_commit = commit(1);
        let commit_root = root.join("commits").join(&exact_commit);
        fs::create_dir_all(&commit_root).unwrap();
        symlink(&root, commit_root.join("escape")).unwrap();
        let index = GalleryIndex {
            schema: GALLERY_SCHEMA.into(),
            current_commit: exact_commit.clone(),
            retention_commits: RETAINED_COMMITS,
            commits: vec![exact_commit.clone()],
        };
        assert!(validate_existing_tree(&root, &index)
            .unwrap_err()
            .contains("symlink"));
        assert!(root.join("commits").join(exact_commit).is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unindexed_history_refuses_before_any_indexed_eviction() {
        let root = temporary_root("unindexed-retention");
        let exact_commit = commit(1);
        fs::create_dir_all(root.join("commits").join(&exact_commit)).unwrap();
        fs::create_dir_all(root.join("commits").join(commit(2))).unwrap();
        let index = GalleryIndex {
            schema: GALLERY_SCHEMA.into(),
            current_commit: exact_commit.clone(),
            retention_commits: RETAINED_COMMITS,
            commits: vec![exact_commit.clone()],
        };
        assert!(validate_existing_tree(&root, &index)
            .unwrap_err()
            .contains("unindexed commit"));
        assert!(root.join("commits").join(exact_commit).is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_still_oversized_retained_commit_refuses() {
        let root = temporary_root("post-retention-bound");
        let exact_commit = commit(1);
        let commit_root = root.join("commits").join(&exact_commit);
        fs::create_dir_all(&commit_root).unwrap();
        let oversized = fs::File::create(commit_root.join("oversized.evidence")).unwrap();
        oversized.set_len(MAX_GALLERY_BYTES + 1).unwrap();
        let mut index = GalleryIndex {
            schema: GALLERY_SCHEMA.into(),
            current_commit: exact_commit.clone(),
            retention_commits: RETAINED_COMMITS,
            commits: vec![exact_commit.clone()],
        };
        assert!(trim_indexed_history_to_bounds(&root, &mut index, false)
            .unwrap_err()
            .contains("after retention"));
        assert!(root.join("commits").join(exact_commit).is_dir());
        fs::remove_dir_all(root).unwrap();
    }
}
