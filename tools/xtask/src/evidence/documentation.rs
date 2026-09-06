use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::sha256_file;

const CURRENT_PATCHBAY: &str = "https://dancxjo.github.io/conduit/current/patchbay/";
const MAX_MARKDOWN_FILES: usize = 256;
const MAX_MARKDOWN_BYTES: u64 = 4 * 1024 * 1024;

const REFERENCES: &[Reference] = &[
    Reference {
        document: "docs/visual-evidence.md",
        scenario: "overview",
    },
    Reference {
        document: "docs/visual-evidence.md",
        scenario: "selected-gear",
    },
    Reference {
        document: "docs/visual-evidence.md",
        scenario: "interaction",
    },
    Reference {
        document: "docs/visual-evidence.md",
        scenario: "disconnected",
    },
];

struct Reference {
    document: &'static str,
    scenario: &'static str,
}

pub struct DocumentationReferenceRequest {
    pub workspace_root: PathBuf,
    pub site_root: Option<PathBuf>,
    pub commit: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GalleryIndex {
    schema: String,
    current_commit: String,
    retention_commits: usize,
    commits: Vec<String>,
}

pub fn verify_documentation_references(
    request: &DocumentationReferenceRequest,
) -> Result<(), String> {
    let markdown = read_markdown_tree(&request.workspace_root)?;
    for reference in REFERENCES {
        let document = markdown
            .iter()
            .find(|(path, _)| path == Path::new(reference.document))
            .ok_or_else(|| {
                format!(
                    "required documentation file '{}' is missing",
                    reference.document
                )
            })?;
        let image_url = current_image_url(reference);
        let provenance_url = current_provenance_url(reference);
        if document.1.matches(&image_url).count() != 1
            || document.1.matches(&provenance_url).count() != 1
        {
            return Err(format!(
                "{} must contain exactly one current '{}' image and provenance link",
                reference.document, reference.scenario
            ));
        }
    }

    let expected_current_urls = REFERENCES.len() * 2;
    let actual_current_urls = markdown
        .iter()
        .map(|(_, contents)| contents.matches(CURRENT_PATCHBAY).count())
        .sum::<usize>();
    if actual_current_urls != expected_current_urls {
        return Err(format!(
            "canonical documentation has {actual_current_urls} current Patchbay URLs; expected {expected_current_urls} exact paired references"
        ));
    }
    if markdown
        .iter()
        .any(|(_, contents)| contents.contains("https://dancxjo.github.io/conduit/commits/"))
    {
        return Err("documentation references immutable commit or ephemeral artifact imagery instead of accepted current evidence".into());
    }

    match (&request.site_root, &request.commit) {
        (None, None) => {}
        (Some(site_root), Some(commit)) => verify_built_gallery(site_root, commit)?,
        _ => return Err("site root and exact commit must be provided together".into()),
    }
    println!(
        "verified {} canonical documentation visual references",
        REFERENCES.len()
    );
    Ok(())
}

fn current_image_url(reference: &Reference) -> String {
    format!("{CURRENT_PATCHBAY}{}.png", reference.scenario)
}

fn current_provenance_url(reference: &Reference) -> String {
    format!("{CURRENT_PATCHBAY}{}/", reference.scenario)
}

fn read_markdown_tree(root: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut paths = vec![PathBuf::from("README.md")];
    collect_markdown(&root.join("docs"), root, &mut paths)?;
    if paths.len() > MAX_MARKDOWN_FILES {
        return Err("documentation Markdown file count exceeds its validation bound".into());
    }
    paths
        .into_iter()
        .map(|relative| {
            let path = root.join(&relative);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
            if !metadata.file_type().is_file() || metadata.len() > MAX_MARKDOWN_BYTES {
                return Err(format!(
                    "{} is not one bounded regular file",
                    path.display()
                ));
            }
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            Ok((relative, contents))
        })
        .collect()
}

fn collect_markdown(directory: &Path, root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot inspect documentation tree: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("cannot inspect documentation entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect documentation entry type: {error}"))?;
        if file_type.is_symlink() {
            return Err(format!(
                "documentation tree contains symlink {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_markdown(&entry.path(), root, paths)?;
        } else if file_type.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            paths.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| "documentation path escapes workspace")?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn verify_built_gallery(site_root: &Path, commit: &str) -> Result<(), String> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("documentation verification commit must be one exact SHA".into());
    }
    let index_path = site_root.join("gallery.json");
    let index: GalleryIndex = serde_json::from_slice(
        &fs::read(&index_path)
            .map_err(|error| format!("cannot read built gallery index: {error}"))?,
    )
    .map_err(|error| format!("invalid built gallery index: {error}"))?;
    if index.schema != "conduit.visual-evidence-gallery/v1"
        || index.current_commit != commit
        || index.commits.first().map(String::as_str) != Some(commit)
        || index.retention_commits != 32
    {
        return Err("built gallery does not advertise the exact accepted current commit".into());
    }
    for reference in REFERENCES {
        let current_image = site_root.join(format!("current/patchbay/{}.png", reference.scenario));
        let historical_image = site_root.join(format!(
            "commits/{commit}/patchbay/{}.png",
            reference.scenario
        ));
        if sha256_file(&current_image)? != sha256_file(&historical_image)? {
            return Err(format!(
                "current '{}' image drifted from exact commit evidence",
                reference.scenario
            ));
        }
        let page_path = site_root.join(format!(
            "current/patchbay/{}/index.html",
            reference.scenario
        ));
        let page = fs::read_to_string(&page_path)
            .map_err(|error| format!("cannot read current provenance page: {error}"))?;
        if !page.contains(commit)
            || !page.contains(&format!("<img src=\"../{}.png\"", reference.scenario))
            || !page.contains("Exact provenance")
        {
            return Err(format!(
                "current '{}' page lacks exact image provenance",
                reference.scenario
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_references_are_current_only_and_complete() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        verify_documentation_references(&DocumentationReferenceRequest {
            workspace_root: workspace.to_path_buf(),
            site_root: None,
            commit: None,
        })
        .unwrap();
    }
}
