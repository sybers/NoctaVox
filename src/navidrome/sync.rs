use crate::{
    Database,
    library::{LongSong, SongInfo, LEGAL_EXTENSION},
    navidrome::{
        runtime::block_on,
        subsonic_http::{get_album_get, get_album_list2_get, ping_get},
    },
};
use anyhow::Result;
use std::collections::HashSet;
use submarine::{
    Client,
    api::get_album_list::Order,
    auth::Auth,
    data::{Child, MediaType},
};

/// Bundles a Subsonic [`Client`] (stream URL / bytes) with GET-based JSON calls for compatibility
/// with strict reverse proxies.
pub struct NavidromeClient {
    submarine: Client,
    http: reqwest::Client,
    base_url: String,
    auth: Auth,
}

fn album_artist_name(album: &submarine::data::AlbumWithSongsId3) -> String {
    album
        .base
        .artist
        .clone()
        .unwrap_or_else(|| "[Unknown Artist]".into())
}

fn is_music_track(c: &Child) -> bool {
    if c.is_dir == Some(true) {
        return false;
    }
    match c.typ {
        Some(MediaType::Music) | None => {}
        _ => return false,
    }
    c.suffix
        .as_deref()
        .map(|s| LEGAL_EXTENSION.contains(s.to_lowercase().as_str()))
        .unwrap_or(true)
}

/// Full catalog refresh: clears local DB music tables, then fills from Navidrome.
pub fn sync_library_from_navidrome(client: &NavidromeClient, db: &mut Database) -> Result<()> {
    db.clear_library_catalog()?;

    let mut all_songs: Vec<LongSong> = Vec::new();
    let mut offset = 0usize;
    const PAGE: usize = 300;

    loop {
        let albums: Vec<Child> = block_on(get_album_list2_get(
            &client.http,
            &client.base_url,
            &client.auth,
            Order::AlphabeticalByName,
            Some(PAGE),
            Some(offset),
            None::<String>,
        ))
        .map_err(|e| anyhow::anyhow!("Navidrome getAlbumList2: {e}"))?;

        if albums.is_empty() {
            break;
        }

        for entry in &albums {
            // getAlbumList2 returns one row per album. Navidrome / OpenSubsonic often omit `isDir`;
            // only skip entries that are explicitly not directories (e.g. stray file rows).
            if entry.is_dir == Some(false) {
                continue;
            }
            let full = match block_on(get_album_get(
                &client.http,
                &client.base_url,
                &client.auth,
                &entry.id,
            )) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("skip album {}: {e}", entry.id);
                    continue;
                }
            };

            let album_title = full.base.name.as_str();
            let aa = album_artist_name(&full);

            for song in &full.song {
                if !is_music_track(song) {
                    continue;
                }
                match LongSong::from_navidrome_song(song, album_title, &aa) {
                    Ok(ls) => all_songs.push(ls),
                    Err(e) => eprintln!("skip song {}: {e}", song.id),
                }
            }
        }

        offset += albums.len();
        if albums.len() < PAGE {
            break;
        }
    }

    let mut artist_cache = HashSet::new();
    let mut aa_binding = HashSet::new();

    for song in &all_songs {
        artist_cache.insert(song.get_artist());
        artist_cache.insert(song.album_artist.as_str());
        aa_binding.insert((song.album_artist.as_str(), song.get_album()));
    }

    db.insert_artists(&artist_cache)?;
    db.insert_albums(&aa_binding)?;
    db.insert_songs(&all_songs)?;

    Ok(())
}

pub fn build_client(base_url: &str, username: &str, password: &str) -> Result<NavidromeClient> {
    let url = base_url.trim().trim_end_matches('/');
    anyhow::ensure!(!url.is_empty(), "Navidrome URL is empty");
    let username = username.trim();
    let password = password.trim();
    let auth = submarine::auth::AuthBuilder::new(username, "1.16.1")
        .client_name("NoctaVox")
        .hashed(password);
    Ok(NavidromeClient {
        submarine: Client::new(url, auth.clone()),
        http: reqwest::Client::new(),
        base_url: url.to_string(),
        auth,
    })
}

pub fn ping(client: &NavidromeClient) -> Result<()> {
    let _ = block_on(ping_get(&client.http, &client.base_url, &client.auth))
        .map_err(|e| anyhow::anyhow!("Navidrome ping failed: {e}"))?;
    Ok(())
}

/// Download track bytes (used for playback temp file).
pub fn download_song(client: &NavidromeClient, nav_id: &str) -> Result<Vec<u8>> {
    block_on(client.submarine.stream(
        nav_id,
        None,
        None::<String>,
        None,
        None::<String>,
        None,
        None,
    ))
    .map_err(|e| anyhow::anyhow!("stream failed for {nav_id}: {e}"))
}
