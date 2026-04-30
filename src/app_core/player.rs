use anyhow::{Result, anyhow};
use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    app_config::AppConfig,
    app_core::NoctaVox,
    key_handler::SelectionType,
    library::{FileType, SimpleSong, SongDatabase, SongInfo},
    navidrome,
    playback::ValidatedSong,
    player::{NoctavoxTrack, PlayerEvent},
    ui_state::{LibraryView, Mode},
};

impl NoctaVox {
    pub(crate) fn remove_navidrome_play_temp(&mut self) {
        if let Some(path) = self.navidrome_play_temp.take() {
            let _ = std::fs::remove_file(path);
        }
    }

    fn resolve_playback_path(&self, song: &ValidatedSong) -> Result<PathBuf> {
        if !song.is_navidrome_stream() {
            return Ok(song.path());
        }

        let nav_id = song
            .path
            .strip_prefix(crate::NAV_PATH_PREFIX)
            .ok_or_else(|| anyhow!("invalid Navidrome path"))?;

        let cfg = AppConfig::load()?;
        let client = navidrome::build_client(
            &cfg.nav_url_trimmed(),
            &cfg.nav_username,
            &cfg.nav_password,
        )?;
        // M4A is MP4: `moov` is often at EOF, so partial download fails in Symphonia. Subsonic
        // `format=mp3` asks Navidrome to transcode; temp file must be named `.mp3`.
        let transcode = matches!(song.meta.filetype, FileType::M4A);
        let ext = if transcode {
            "mp3"
        } else {
            song.meta.filetype.as_file_extension()
        };
        let dest = std::env::temp_dir().join(format!("noctavox_play_{}.{}", song.id(), ext));
        navidrome::stream_track_to_file(&client, nav_id, dest.clone(), transcode)?;
        Ok(dest)
    }

    pub(crate) fn play_song(&mut self, song: &ValidatedSong) -> Result<()> {
        self.remove_navidrome_play_temp();

        let path = self.resolve_playback_path(song)?;
        let is_navidrome = song.is_navidrome_stream();
        let track = NoctavoxTrack::new(song.id(), path.clone());

        match self.player.play(track) {
            Ok(()) => {
                if is_navidrome {
                    self.navidrome_play_temp = Some(path);
                }
                Ok(())
            }
            Err(e) => {
                if is_navidrome {
                    let _ = std::fs::remove_file(path);
                }
                Err(e)
            }
        }
    }

    pub(crate) fn play_selected_song(&mut self, count: usize) -> Result<()> {
        match count {
            0 => (),
            x => self.ui.go_to_track(x)?,
        }

        let song = self.ui.get_selected_song()?;
        let selected_idx = self.ui.get_selected_idx()?;

        if self.ui.get_mode() == &Mode::Queue {
            self.remove_song()?;
        }

        // Auto-advance at end of track requires upcoming songs in the queue; enqueue the rest of
        // the current list (album tracks, playlist, Power library, search results).
        if Self::queue_tail_for_autoplay(self.ui.get_mode()) {
            self.ui.playback.clear_queue();
            let tail = self.ui.legal_songs_tail(selected_idx);
            if !tail.is_empty() {
                self.ui.playback.enqueue_multi(&tail)?;
            }
        }

        let validated = ValidatedSong::new(&song)?;
        self.play_song(&validated)?;
        self.force_sync()?;

        Ok(())
    }

    fn queue_tail_for_autoplay(mode: &Mode) -> bool {
        matches!(mode, Mode::Power | Mode::Search | Mode::Library(_))
    }

    pub(crate) fn play_next(&mut self) -> Result<()> {
        let (delta, next) = self.ui.playback.advance();

        match next {
            Some(song) => {
                self.play_song(&song)?;
                self.sync_player(&delta);
            }
            None => self.player.stop()?,
        }
        self.ui.set_legal_songs();

        Ok(())
    }

    pub(crate) fn play_prev(&mut self) -> Result<()> {
        let (delta, popped) = self
            .ui
            .playback
            .pop_previous()?
            .ok_or_else(|| anyhow!("End of history!"))?;

        self.play_song(&popped)?;
        self.sync_player(&delta);
        self.ui.set_legal_songs();
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.ui.playback.clear_queue();
        self.player.stop()?;
        self.remove_navidrome_play_temp();
        Ok(())
    }

    pub fn remove_song(&mut self) -> Result<()> {
        match self.ui.get_mode() {
            Mode::Queue => match self.ui.multi_select_empty() {
                true => self.remove_from_queue()?,
                false => self.remove_from_queue_multi()?,
            },
            Mode::Library(LibraryView::Playlists) => match self.ui.multi_select_empty() {
                true => self.ui.remove_from_playlist()?,
                false => self.ui.remove_from_playlist_multi()?,
            },
            _ => {}
        }
        self.ui.set_legal_songs();
        Ok(())
    }

    pub fn queue_handler(&mut self, selection: Option<Arc<SimpleSong>>) -> Result<()> {
        if !self.ui.multi_select_empty() {
            return self.queue_selection(SelectionType::Multi, false);
        }

        let ss = match selection {
            Some(s) => s,
            None => self.ui.get_selected_song()?,
        };

        match self.player.is_stopped() {
            true => {
                let validated = ValidatedSong::new(&ss)?;
                self.play_song(&validated)?;
            }
            false => self.queue_song(&ss)?,
        }

        self.ui.set_legal_songs();
        Ok(())
    }

    pub(super) fn handle_player_events(&mut self, event: PlayerEvent) -> Result<()> {
        match event {
            PlayerEvent::TrackStarted((this_song, was_gapless)) => {
                let return_id = this_song.id();

                if was_gapless {
                    self.advance_to_next_gapless();
                }
                let song = self.library.get_song_by_id(return_id).cloned();
                self.ui.set_now_playing(song);

                if let Some(song) = self.library.get_song_by_id(return_id).cloned() {
                    song.update_play_count()?;
                    self.ui.clear_waveform();
                    self.ui.request_waveform(&song);

                    if let Some(mc) = self.media_controls.as_mut() {
                        mc.update_metadata(
                            song.get_title(),
                            song.get_artist(),
                            song.get_album(),
                            song.get_duration(),
                        );
                        mc.set_playing(Duration::ZERO);
                    }
                }

                Ok(())
            }
            PlayerEvent::PlaybackStopped => {
                let (delta, next) = self.ui.playback.advance();

                if let Some(song) = next {
                    self.play_song(&song)?;
                    self.sync_player(&delta);
                    return Ok(());
                }

                self.remove_navidrome_play_temp();

                if let Some(mc) = self.media_controls.as_mut() {
                    mc.set_stopped();
                }

                if self.ui.get_mode() == Mode::Fullscreen {
                    self.ui.revert_fullscreen();
                }
                self.ui.playback.set_now_playing(None);
                self.ui.clear_waveform();
                self.ui.set_legal_songs();
                Ok(())
            }
            PlayerEvent::Error(e) => {
                self.ui.set_error(anyhow!(e));
                Ok(())
            }
        }
    }
}
