use crate::{
    Library,
    app_config::{AppConfig, LibraryMode},
    app_core::NoctaVox,
    navidrome,
    ui_state::{PopupType, SettingsMode, SetupMode},
};
use anyhow::Result;
use std::sync::Arc;

impl NoctaVox {
    pub(crate) fn setup_wizard_confirm(&mut self) -> Result<()> {
        match &self.ui.popup.current.clone() {
            PopupType::Setup(SetupMode::ChooseKind) => self.setup_choose_kind(),
            PopupType::Setup(SetupMode::NavUrl) => self.setup_nav_url(),
            PopupType::Setup(SetupMode::NavUser) => self.setup_nav_user(),
            PopupType::Setup(SetupMode::NavPassword) => self.setup_nav_password(),
            _ => Ok(()),
        }
    }

    fn setup_choose_kind(&mut self) -> Result<()> {
        let idx = self
            .ui
            .popup
            .selection
            .selected()
            .unwrap_or(0);
        match idx {
            0 => {
                let mut cfg = AppConfig::load()?;
                cfg.library_mode = LibraryMode::Local;
                cfg.save()?;
                self.reload_library_arc()?;
                self.ui
                    .show_popup(PopupType::Settings(SettingsMode::AddRoot));
            }
            1 => {
                self.ui.show_popup(PopupType::Setup(SetupMode::NavUrl));
            }
            _ => {}
        }
        Ok(())
    }

    fn setup_nav_url(&mut self) -> Result<()> {
        let url = self.ui.get_popup_string();
        if url.is_empty() {
            return Ok(());
        }
        self.ui.setup_draft.nav_base_url = url;
        self.ui.show_popup(PopupType::Setup(SetupMode::NavUser));
        Ok(())
    }

    fn setup_nav_user(&mut self) -> Result<()> {
        let user = self.ui.get_popup_string();
        if user.is_empty() {
            return Ok(());
        }
        self.ui.setup_draft.nav_username = user;
        self.ui.show_popup(PopupType::Setup(SetupMode::NavPassword));
        Ok(())
    }

    fn setup_nav_password(&mut self) -> Result<()> {
        let pass = self.ui.get_popup_string();
        if pass.is_empty() {
            return Ok(());
        }

        let mut cfg = AppConfig::load()?;
        cfg.library_mode = LibraryMode::Navidrome;
        cfg.nav_base_url = self.ui.setup_draft.nav_base_url.trim().to_string();
        cfg.nav_username = self.ui.setup_draft.nav_username.trim().to_string();
        cfg.nav_password = pass;
        cfg.onboarding_complete = true;
        cfg.save()?;

        let client = navidrome::build_client(
            &cfg.nav_url_trimmed(),
            &cfg.nav_username,
            &cfg.nav_password,
        )?;
        navidrome::ping(&client)?;

        let mut lib = Library::init();
        lib.build_library()?;
        self.library = Arc::new(lib);
        self.ui.sync_library(Arc::clone(&self.library))?;
        self.ui.close_popup();
        self.ui.setup_draft = crate::ui_state::SetupWizardDraft::default();
        let _ = crate::app_config::security_readme_if_missing();
        Ok(())
    }

    pub(crate) fn reload_library_arc(&mut self) -> Result<()> {
        let lib = Library::init();
        self.library = Arc::new(lib);
        self.ui.sync_library(Arc::clone(&self.library))?;
        Ok(())
    }
}
