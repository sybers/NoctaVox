use ratatui::{crossterm::event::KeyEvent, widgets::ListState};
use ratatui_textarea::TextArea;

use crate::{
    get_random_playlist_idea,
    ui_state::{Pane, SettingsMode, UiState, new_textarea, playlist::PlaylistAction},
};

#[derive(PartialEq, Clone)]
pub enum SetupMode {
    ChooseKind,
    NavUrl,
    NavUser,
    NavPassword,
}

#[derive(PartialEq, Clone)]
pub enum PopupType {
    None,
    Stats,
    Error(String),
    Settings(SettingsMode),
    Playlist(PlaylistAction),
    ThemeManager,
    Setup(SetupMode),
}

pub struct PopupState {
    pub current: PopupType,
    pub input: TextArea<'static>,
    pub selection: ListState,
    pub cached: Pane,
}

impl PopupState {
    pub(crate) fn new() -> PopupState {
        PopupState {
            current: PopupType::None,
            input: new_textarea(""),
            selection: ListState::default(),
            cached: Pane::Popup,
        }
    }

    fn open(&mut self, popup: PopupType) {
        match &popup {
            PopupType::Playlist(PlaylistAction::Rename)
            | PopupType::Playlist(PlaylistAction::Create)
            | PopupType::Playlist(PlaylistAction::CreateWithSongs) => {
                let placeholder = get_random_playlist_idea();
                self.input.set_placeholder_text(format!(" {placeholder} "));
                self.input.clear();
            }
            PopupType::Settings(SettingsMode::ViewRoots) => {
                self.input.clear();
            }
            PopupType::Settings(SettingsMode::AddRoot) => {
                self.input
                    .set_placeholder_text(" Enter path to directory: ");
                self.input.clear();
            }
            PopupType::Setup(SetupMode::ChooseKind) => {
                self.input.clear();
                self.selection.select(Some(0));
            }
            PopupType::Setup(SetupMode::NavUrl) => {
                self.input
                    .set_placeholder_text(" https://navidrome.example.com ");
                self.input.clear();
            }
            PopupType::Setup(SetupMode::NavUser) => {
                self.input.set_placeholder_text(" username ");
                self.input.clear();
            }
            PopupType::Setup(SetupMode::NavPassword) => {
                self.input.set_placeholder_text(" password or token ");
                self.input.clear();
            }

            _ => (),
        }
        self.current = popup
    }

    pub fn is_open(&self) -> bool {
        self.current != PopupType::None
    }

    fn close(&mut self) -> Pane {
        self.current = PopupType::None;
        self.input.clear();

        self.cached.clone()
    }

    fn set_cached_pane(&mut self, pane: Pane) {
        self.cached = pane
    }
}

impl UiState {
    pub fn show_popup(&mut self, popup: PopupType) {
        self.popup.open(popup);
        if self.popup.cached == Pane::Popup {
            let current_pane = self.get_pane().clone();
            self.popup.set_cached_pane(current_pane);
            self.set_pane(Pane::Popup);
        }
    }

    pub fn get_popup_string(&self) -> String {
        self.popup.input.lines()[0].trim().to_string()
    }

    pub fn close_popup(&mut self) {
        let pane = self.popup.close();
        self.popup.cached = Pane::Popup;
        self.set_pane(pane);
    }

    pub fn process_popup_input(&mut self, key: &KeyEvent) {
        self.popup.input.input(*key);
    }
}
