use crate::{
    Library,
    key_handler::KeyBuffer,
    media_controls::MediaControlsHandle,
    player::PlayerHandle,
    ui_state::UiState,
};
use crossbeam::channel::Receiver;
use std::{path::PathBuf, sync::Arc};

mod app;
mod key_events;
mod library;
mod onboarding;
mod playback;
mod player;
mod select;

pub use key_events::key_loop;

pub struct NoctaVox {
    library: Arc<Library>,
    pub(crate) ui: UiState,
    player: PlayerHandle,
    key_buffer: KeyBuffer,
    library_refresh_rec: Option<Receiver<LibraryRefreshProgress>>,
    media_controls: Option<MediaControlsHandle>,
    media_sync_tick: u32,
    /// Last Navidrome temp cache file under `std::env::temp_dir()`; removed on track change / stop.
    navidrome_play_temp: Option<PathBuf>,
}

pub enum LibraryRefreshProgress {
    Scanning {
        progress: u8,
    },
    Processing {
        progress: u8,
        current: usize,
        total: usize,
    },
    UpdatingDatabase {
        progress: u8,
    },
    Rebuilding {
        progress: u8,
    },
    Complete(crate::Library),
    Error(String),
}
