pub mod discovery;
pub mod registry;
pub mod view;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Script {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub source: String,
}

impl Script {
    pub fn command(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub env_files: Vec<PathBuf>,
    #[serde(default)]
    pub scripts: Vec<Script>,
    #[serde(default)]
    pub selected_script: Option<String>,
    #[serde(default)]
    pub selected_env: Option<PathBuf>,
}

impl Project {
    pub fn script(&self) -> Option<&Script> {
        self.scripts
            .iter()
            .find(|s| Some(&s.name) == self.selected_script.as_ref())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub projects: Vec<Project>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            projects: vec![],
        }
    }
}
