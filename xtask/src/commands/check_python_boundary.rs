use std::{fs, path::Path, process::Command};

pub fn run(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["ls-files", "--stage", "--other", "--exclude-standard"])
        .current_dir(workspace_root)
        .output()?;

    if !output.status.success() {
        return Err("git ls-files failed".into());
    }

    let files_text = String::from_utf8_lossy(&output.stdout);
    let mut tracked_paths = Vec::new();
    for line in files_text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            tracked_paths.push(parts[3..].join(" "));
        } else if !line.is_empty() {
            tracked_paths.push(line.to_string());
        }
    }

    let mut violations = Vec::new();

    let manifest_names = [
        "requirements.txt",
        "pyproject.toml",
        "Pipfile",
        "Pipfile.lock",
        "setup.py",
        "setup.cfg",
        "poetry.lock",
        "tox.ini",
    ];

    for rel_path in &tracked_paths {
        let path = workspace_root.join(rel_path);
        if !path.is_file() {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // 1. Check *.py
        if file_name.ends_with(".py") {
            violations.push(format!(
                "{rel_path}: repository-owned Python script file is forbidden"
            ));
            continue;
        }

        // 2. Check Python manifests
        if manifest_names.contains(&file_name) {
            violations.push(format!(
                "{rel_path}: Python dependency manifest file is forbidden"
            ));
            continue;
        }

        // Read content for shebang and invocation checks
        if let Ok(content) = fs::read_to_string(&path) {
            let mut lines = content.lines();

            // 3. Check shebang
            if let Some(first_line) = lines.next() {
                if first_line.starts_with("#!") && first_line.to_lowercase().contains("python") {
                    violations.push(format!("{rel_path}:1: Python shebang is forbidden"));
                }
            }

            // 4. Check CI, shell, build script invocations of python/pip/pytest
            let is_ci_or_script = rel_path.starts_with(".github/")
                || rel_path.ends_with(".sh")
                || rel_path.ends_with(".bash")
                || rel_path == "Justfile"
                || rel_path == "Makefile"
                || rel_path.ends_with(".mjs")
                || rel_path.ends_with(".js");

            if is_ci_or_script {
                for (idx, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('#') || trimmed.starts_with("//") {
                        continue;
                    }
                    // Match command invocations of python, python3, pip, pip3, pytest
                    for tool in ["python", "python3", "pip", "pip3", "pytest", "tox", "nox"] {
                        let is_invoked = trimmed.starts_with(tool)
                            || trimmed.contains(&format!(" {tool} "))
                            || trimmed.contains(&format!("; {tool} "))
                            || trimmed.contains(&format!("&& {tool} "))
                            || trimmed.contains(&format!("|| {tool} "))
                            || trimmed.contains(&format!("`{tool} "))
                            || trimmed.contains(&format!("$({tool} "));

                        if is_invoked {
                            // Check that it's not part of prose or string comments
                            violations.push(format!(
                                "{rel_path}:{}: forbidden invocation of '{tool}': {trimmed}",
                                idx + 1
                            ));
                        }
                    }
                }
            }
        }
    }

    if !violations.is_empty() {
        eprintln!("Python Boundary Violations Detected:");
        for v in &violations {
            eprintln!("  - {v}");
        }
        return Err(format!("Found {} Python boundary violation(s)", violations.len()).into());
    }

    println!(
        "Python boundary check passed: zero repository-owned Python scripts or invocations detected."
    );
    Ok(())
}
