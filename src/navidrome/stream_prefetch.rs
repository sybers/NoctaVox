//! Navidrome HTTP stream into a temp path for playback.
//!
//! - **Unix**: named pipe (FIFO) + background writer — playback can start while data streams; reads
//!   block instead of hitting a premature EOF. If `mkfifo` fails, falls back to a full download
//!   before returning.
//! - **Other platforms**: full download to a regular file before returning.

use std::{
    io::Write,
    path::{Path, PathBuf},
    thread,
};

use anyhow::{Result, anyhow};

#[cfg(unix)]
use std::process::Command;

use crate::navidrome::NavidromeClient;

/// Prepares `dest` for playback (FIFO stream on Unix when possible, else full file).
///
/// `transcode_to_mp3`: pass-through to [NavidromeClient::stream_url_for_track] (M4A → MP3).
pub fn stream_track_to_file(
    client: &NavidromeClient,
    nav_id: &str,
    dest: PathBuf,
    transcode_to_mp3: bool,
) -> Result<()> {
    #[cfg(unix)]
    {
        stream_unix(client, nav_id, dest, transcode_to_mp3)
    }
    #[cfg(not(unix))]
    {
        stream_wait_full_file(client, nav_id, dest, transcode_to_mp3)
    }
}

#[cfg(unix)]
fn stream_unix(
    client: &NavidromeClient,
    nav_id: &str,
    dest: PathBuf,
    transcode_to_mp3: bool,
) -> Result<()> {
    let _ = std::fs::remove_file(&dest);
    if create_fifo(&dest).is_ok() {
        stream_fifo_fire_and_forget(client, nav_id, dest, transcode_to_mp3)
    } else {
        stream_wait_full_file(client, nav_id, dest, transcode_to_mp3)
    }
}

/// Spawn writer only; main thread returns immediately so `play` can open the FIFO read end.
#[cfg(unix)]
fn stream_fifo_fire_and_forget(
    client: &NavidromeClient,
    nav_id: &str,
    dest: PathBuf,
    transcode_to_mp3: bool,
) -> Result<()> {
    let url = client.stream_url_for_track(nav_id, transcode_to_mp3)?;
    let dest_clone = dest.clone();

    thread::spawn(move || {
        let run = || -> Result<(), String> {
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .open(&dest_clone)
                .map_err(|e| format!("fifo write open: {e}"))?;

            copy_http_body_to_writer(&mut out, &url)
        };

        if let Err(e) = run() {
            eprintln!("Navidrome FIFO writer: {e}");
        }
    });

    Ok(())
}

#[cfg(unix)]
fn create_fifo(path: &Path) -> Result<()> {
    let status = Command::new("mkfifo")
        .arg(path)
        .status()
        .map_err(|e| anyhow!("mkfifo: {e}"))?;
    if !status.success() {
        return Err(anyhow!("mkfifo failed for {}", path.display()));
    }
    Ok(())
}

fn copy_http_body_to_writer<W: Write>(mut out: W, url: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| format!("reqwest blocking: {e}"))?;

    let mut resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "stream HTTP {} {}",
            resp.status(),
            resp.status().canonical_reason().unwrap_or("")
        ));
    }

    std::io::copy(&mut resp, &mut out).map_err(|e| format!("stream copy: {e}"))?;
    Ok(())
}

/// Download the entire stream, blocking until done (Windows, Unix fallback, mkfifo failure).
fn stream_wait_full_file(
    client: &NavidromeClient,
    nav_id: &str,
    dest: PathBuf,
    transcode_to_mp3: bool,
) -> Result<()> {
    let url = client.stream_url_for_track(nav_id, transcode_to_mp3)?;
    let (tx, rx) = crossbeam::channel::bounded(1);

    thread::spawn(move || {
        let r = (|| -> Result<(), String> {
            let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
            copy_http_body_to_writer(&mut out, &url)
        })();
        let _ = tx.send(r);
    });

    match rx.recv() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(anyhow!(e)),
        Err(_) => Err(anyhow!("Navidrome stream failed before playback could start")),
    }
}
