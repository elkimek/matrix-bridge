use crate::error::{BridgeError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Configuration for the Matrix bridge.
/// Backwards-compatible with the Python version's ~/.matrix-bridge/config.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub homeserver: String,
    pub user_id: String,

    #[serde(default = "default_device_name")]
    pub device_name: String,

    #[serde(default = "default_store_path")]
    pub store_path: String,

    #[serde(default = "default_trust_mode")]
    pub trust_mode: TrustMode,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_room: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mention: Option<String>,

    /// Optional field: trigger channel notifications when this pattern is mentioned.
    /// Defaults to the bridge's own user_id if not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_on_mention: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TrustMode {
    Tofu,
    All,
    Explicit,
}

/// Saved session credentials (access_token, user_id, device_id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub user_id: String,
    pub device_id: String,
}

fn default_device_name() -> String {
    "matrix-bridge".to_string()
}

fn default_store_path() -> String {
    default_dir()
        .join("store")
        .to_string_lossy()
        .into_owned()
}

fn default_trust_mode() -> TrustMode {
    TrustMode::Tofu
}

/// Returns ~/.matrix-bridge
pub fn default_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".matrix-bridge")
}

/// Returns the config file path: ~/.matrix-bridge/config.json
pub fn config_path() -> PathBuf {
    default_dir().join("config.json")
}

/// Returns the credentials file path: ~/.matrix-bridge/store/credentials.json
pub fn credentials_path(config: &Config) -> PathBuf {
    PathBuf::from(&config.store_path).join("credentials.json")
}

/// Ensure a directory exists with 0700 permissions.
fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Write a file with 0600 permissions.
fn write_secure(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, contents)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

impl Config {
    /// Load config from the default path.
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Err(BridgeError::Config(format!(
                "Config not found at {} — run `matrix-bridge setup`",
                path.display()
            )));
        }
        let data = fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&data)?;
        Ok(config)
    }

    /// Save config to the default path with secure permissions.
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        let data = serde_json::to_string_pretty(self)?;
        write_secure(&path, &data)?;
        Ok(())
    }

    /// Ensure the store directory exists.
    pub fn ensure_store_dir(&self) -> Result<()> {
        ensure_dir(Path::new(&self.store_path))
    }

    /// Get the mention pattern for channel notifications.
    /// Falls back to the local part of user_id (e.g., "@bot:matrix.org" -> "bot").
    pub fn mention_pattern(&self) -> String {
        if let Some(ref pattern) = self.notify_on_mention {
            return pattern.clone();
        }
        // Extract local part from @user:server
        self.user_id
            .strip_prefix('@')
            .and_then(|s| s.split(':').next())
            .unwrap_or(&self.user_id)
            .to_string()
    }
}

impl Credentials {
    /// Load credentials from the store.
    pub fn load(config: &Config) -> Result<Self> {
        let path = credentials_path(config);
        if !path.exists() {
            return Err(BridgeError::NoSession);
        }
        let data = fs::read_to_string(&path)?;
        let creds: Credentials = serde_json::from_str(&data)?;
        Ok(creds)
    }

    /// Save credentials to the store with secure permissions.
    pub fn save(&self, config: &Config) -> Result<()> {
        let path = credentials_path(config);
        let data = serde_json::to_string_pretty(self)?;
        write_secure(&path, &data)?;
        Ok(())
    }
}
