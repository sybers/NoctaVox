//! Application library mode and Navidrome connection settings (session_state).

use crate::{Database, CONFIG_DIRECTORY};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

pub const SESSION_LIBRARY_MODE: &str = "app_library_mode";
pub const SESSION_NAV_URL: &str = "app_nav_base_url";
pub const SESSION_NAV_USER: &str = "app_nav_username";
pub const SESSION_NAV_PASSWORD: &str = "app_nav_password";
pub const SESSION_ONBOARDING_DONE: &str = "app_onboarding_complete";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryMode {
    #[default]
    Local,
    Navidrome,
}

impl LibraryMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            LibraryMode::Local => "local",
            LibraryMode::Navidrome => "navidrome",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(LibraryMode::Local),
            "navidrome" => Some(LibraryMode::Navidrome),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub library_mode: LibraryMode,
    pub nav_base_url: String,
    pub nav_username: String,
    pub nav_password: String,
    pub onboarding_complete: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            library_mode: LibraryMode::Local,
            nav_base_url: String::new(),
            nav_username: String::new(),
            nav_password: String::new(),
            onboarding_complete: false,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let mut db = Database::open()?;
        let mode = db
            .get_session_value(SESSION_LIBRARY_MODE)?
            .and_then(|s| LibraryMode::parse(&s))
            .unwrap_or(LibraryMode::Local);

        let mut onboarding_complete = db
            .get_session_value(SESSION_ONBOARDING_DONE)?
            .map(|v| v == "1")
            .unwrap_or(false);

        // Existing installs: had music roots before onboarding flag existed.
        if !onboarding_complete && mode == LibraryMode::Local {
            let roots = db.get_roots()?.len();
            if roots > 0 {
                onboarding_complete = true;
                db.set_session_value(SESSION_ONBOARDING_DONE, "1")?;
            }
        }

        Ok(Self {
            library_mode: mode,
            nav_base_url: db.get_session_value(SESSION_NAV_URL)?.unwrap_or_default(),
            nav_username: db.get_session_value(SESSION_NAV_USER)?.unwrap_or_default(),
            nav_password: db.get_session_value(SESSION_NAV_PASSWORD)?.unwrap_or_default(),
            onboarding_complete,
        })
    }

    pub fn save(&self) -> Result<()> {
        let mut db = Database::open()?;
        db.set_session_value(SESSION_LIBRARY_MODE, self.library_mode.as_str())?;
        db.set_session_value(SESSION_NAV_URL, &self.nav_base_url)?;
        db.set_session_value(SESSION_NAV_USER, &self.nav_username)?;
        db.set_session_value(SESSION_NAV_PASSWORD, &self.nav_password)?;
        db.set_session_value(
            SESSION_ONBOARDING_DONE,
            if self.onboarding_complete { "1" } else { "0" },
        )?;
        Ok(())
    }

    /// Local library is usable (onboarding done and at least one root), or Navidrome (onboarding + non-empty URL).
    pub fn is_library_ready(&self, has_local_roots: bool) -> bool {
        if !self.onboarding_complete {
            return false;
        }
        match self.library_mode {
            LibraryMode::Local => has_local_roots,
            LibraryMode::Navidrome => {
                !self.nav_base_url.is_empty()
                    && !self.nav_username.is_empty()
                    && !self.nav_password.is_empty()
            }
        }
    }

    pub fn nav_url_trimmed(&self) -> String {
        self.nav_base_url.trim_end_matches('/').to_string()
    }
}

/// Short note beside config dir (password is stored in plaintext in the DB for this MVP).
pub fn security_readme_if_missing() -> Result<()> {
    let dir = dirs::config_dir()
        .context("config dir")?
        .join(CONFIG_DIRECTORY);
    fs::create_dir_all(&dir)?;
    let path = dir.join("NAVIDROME_SECURITY.txt");
    if path.exists() {
        return Ok(());
    }
    fs::write(
        &path,
        "Navidrome mode (MVP): credentials are stored in plaintext in the local SQLite database.\n\
         Do not use on shared machines. Future versions may use the OS keychain.\n",
    )?;
    Ok(())
}
