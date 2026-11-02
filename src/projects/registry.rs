use super::Config;
use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const APP_DIR: &str = "envoke";
// Registry location used before the rename; read once so existing projects carry over.
const LEGACY_DIR: &str = "env-management";

pub struct Store {
    pub dir: PathBuf,
    _lock: File,
}

impl Store {
    pub fn open(override_dir: Option<PathBuf>) -> Result<(Self, Config)> {
        let base = BaseDirs::new().map(|b| b.config_dir().to_path_buf());
        let dir = match (&override_dir, &base) {
            (Some(dir), _) => dir.clone(),
            (None, Some(base)) => base.join(APP_DIR),
            (None, None) => PathBuf::from(".config").join(APP_DIR),
        };
        if !dir.exists() {
            fs::create_dir_all(&dir).context("Could not create configuration directory")?;
            // Only a folder this app created is made private; a user-supplied folder keeps its permissions.
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("instance.lock"))?;
        lock.try_lock_exclusive()
            .context("Another envoke instance is using this configuration")?;
        let path = dir.join("config.json");
        if !path.exists()
            && override_dir.is_none()
            && let Some(legacy) = base.map(|b| b.join(LEGACY_DIR).join("config.json"))
            && legacy.is_file()
        {
            atomic_write(&path, &fs::read(&legacy)?, 0o600)
                .context("Could not copy the env-management registry")?;
        }
        let config: Config = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)
                .context("Invalid config.json; original file was left untouched")?
        } else {
            Config::default()
        };
        if config.version != 1 {
            bail!("Unsupported config version {}", config.version);
        }
        Ok((Self { dir, _lock: lock }, config))
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        atomic_write(
            &self.dir.join("config.json"),
            &serde_json::to_vec_pretty(config)?,
            0o600,
        )
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path.parent().context("File has no parent directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .context("Could not replace file atomically")?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
