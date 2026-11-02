use super::{Project, Script};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

const SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".cache",
    "vendor",
    "coverage",
    ".turbo",
];
const MAX_DEPTH: usize = 5;
const DEFAULT_SCRIPTS: [&str; 3] = ["dev", "start", "cargo run"];

pub fn expand_path(input: &str) -> PathBuf {
    if (input == "~" || input.starts_with("~/"))
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(input.strip_prefix("~/").unwrap_or(""));
    }
    PathBuf::from(input)
}

pub fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.") || name.ends_with(".env")
}

pub fn create(name: &str, path: &str) -> Result<Project> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Project name cannot be empty");
    }
    let path = expand_path(path.trim())
        .canonicalize()
        .context("Project folder does not exist or is inaccessible")?;
    if !path.is_dir() {
        bail!("Choose a project folder, not a file");
    }
    let mut project = Project {
        name: name.into(),
        path,
        env_files: vec![],
        scripts: vec![],
        selected_script: None,
        selected_env: None,
    };
    refresh(&mut project)?;
    Ok(project)
}

pub fn refresh(project: &mut Project) -> Result<()> {
    fs::read_dir(&project.path).context("Could not scan project folder")?;
    let mut files = vec![];
    scan(&project.path, &project.path, 0, &mut files);
    files.sort();
    let scripts = scripts(&project.path)?;
    if !scripts
        .iter()
        .any(|s| Some(&s.name) == project.selected_script.as_ref())
    {
        project.selected_script = DEFAULT_SCRIPTS.iter().find_map(|name| {
            scripts
                .iter()
                .find(|s| s.name == *name)
                .map(|s| s.name.clone())
        });
    }
    if !files
        .iter()
        .any(|p| Some(p) == project.selected_env.as_ref())
    {
        project.selected_env = files.first().cloned();
    }
    project.env_files = files;
    project.scripts = scripts;
    Ok(())
}

// Unreadable subfolders (for example a container's data folder owned by another user) are skipped, not fatal.
fn scan(root: &Path, folder: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if kind.is_dir() && !SKIP.contains(&name.as_str()) {
            scan(root, &entry.path(), depth + 1, output);
        } else if kind.is_file()
            && is_env_name(&name)
            && !name.ends_with(".swp")
            && !name.ends_with(".bak")
            && let Ok(relative) = entry.path().strip_prefix(root)
        {
            output.push(relative.to_path_buf());
        }
        // Symlinks are intentionally excluded: edits must stay inside the project.
    }
}

fn scripts(root: &Path) -> Result<Vec<Script>> {
    let mut result = vec![];
    let package = root.join("package.json");
    if package.is_file() {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(package)?).context("Could not parse package.json")?;
        let manager = package_manager(root, &value);
        if let Some(entries) = value.get("scripts").and_then(|v| v.as_object()) {
            for (name, command) in entries {
                if command.is_string() {
                    result.push(Script {
                        name: name.clone(),
                        program: manager.clone(),
                        args: vec!["run".into(), name.clone()],
                        source: "package.json".into(),
                    });
                }
            }
        }
    }
    if root.join("Cargo.toml").is_file() {
        let _: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)
            .context("Could not parse Cargo.toml")?;
        for command in ["run", "check", "test", "build"] {
            result.push(Script {
                name: format!("cargo {command}"),
                program: "cargo".into(),
                args: vec![command.into()],
                source: "Cargo.toml".into(),
            });
        }
    }
    if root.join("pyproject.toml").is_file() {
        let value: toml::Value = toml::from_str(&fs::read_to_string(root.join("pyproject.toml"))?)
            .context("Could not parse pyproject.toml")?;
        if let Some(table) = value
            .get("project")
            .and_then(|v| v.get("scripts"))
            .and_then(|v| v.as_table())
        {
            for name in table.keys() {
                result.push(Script {
                    name: format!("uv: {name}"),
                    program: "uv".into(),
                    args: vec!["run".into(), name.clone()],
                    source: "pyproject.toml".into(),
                });
            }
        }
    }
    result.sort_by_key(|s| (s.name != "dev", s.name != "start", s.name.clone()));
    Ok(result)
}

fn package_manager(root: &Path, package: &serde_json::Value) -> String {
    let declared = package
        .get("packageManager")
        .and_then(|v| v.as_str())
        .and_then(|s| s.split('@').next())
        .filter(|s| ["npm", "pnpm", "yarn", "bun"].contains(s));
    if let Some(manager) = declared {
        return manager.into();
    }
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if root.join("yarn.lock").exists() {
        "yarn"
    } else if root.join("bun.lock").exists() || root.join("bun.lockb").exists() {
        "bun"
    } else {
        "npm"
    }
    .into()
}
