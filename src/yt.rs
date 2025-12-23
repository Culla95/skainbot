use std::process::Stdio;
use tokio::process::Command;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use crate::queue::Track;
use serenity::model::id::UserId;
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct YtDlpJson {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub webpage_url: Option<String>,
    pub thumbnails: Option<Vec<Thumbnail>>,
    // For playlists
    pub entries: Option<Vec<YtDlpJson>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Thumbnail {
    pub url: String,
}

pub async fn search_query(query: &str, requester: UserId) -> Result<Vec<Track>> {
    // If it's a URL, use it directly, otherwise use ytsearch:
    let arg = if query.starts_with("http") {
        query.to_string()
    } else {
        format!("ytsearch1:{}", query)
    };

    let output = Command::new("yt-dlp")
        .arg("--dump-single-json")
        .arg("--no-warnings")
        .arg("--skip-download")
        .arg("--flat-playlist") // Don't recurse into playlists massively if not needed, but for "play <playlist>" we might want to.
             // Actually, the requirements say "expand playlist". 
             // If we use --flat-playlist we get entries with title/id/url but not full metadata.
             // For performance/speed on large playlists, flat-playlist is better, getting full metadata just in time.
             // But for simplicity let's try to get what we can.
             // Re-reading requirements: "Si es URL que corresponde a playlist: expandir ... encolar todas".
        .arg(&arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // Capture stderr to debug if fails
        .spawn()
        .context("Failed to spawn yt-dlp")?
        .wait_with_output()
        .await
        .context("Failed to wait for yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: YtDlpJson = serde_json::from_str(&stdout).context("Failed to parse yt-dlp JSON")?;

    let mut tracks = Vec::new();

    if let Some(entries) = json.entries {
        // It's a playlist or search result
        for entry in entries {
             let url = entry.webpage_url.unwrap_or_else(|| format!("https://youtube.com/watch?v={}", entry.id));
             tracks.push(Track {
                 title: entry.title,
                 channel: entry.uploader.unwrap_or("Unknown".to_string()),
                 url,
                 duration: entry.duration.map(|d| Duration::from_secs_f64(d)),
                 requester,
                 thumbnail_url: entry.thumbnails.and_then(|t| t.first().map(|x| x.url.clone())),
             });
        }
    } else {
        // Single video
        let url = json.webpage_url.unwrap_or_else(|| format!("https://youtube.com/watch?v={}", json.id));
        tracks.push(Track {
            title: json.title,
            channel: json.uploader.unwrap_or("Unknown".to_string()),
            url,
            duration: json.duration.map(|d| Duration::from_secs_f64(d)),
            requester,
            thumbnail_url: json.thumbnails.and_then(|t| t.first().map(|x| x.url.clone())),
        });
    }

    Ok(tracks)
}

// Function to get direct audio URL for streaming (Just-in-time)
pub async fn get_direct_url(url: &str) -> Result<String> {
    let output = Command::new("yt-dlp")
        .arg("-f")
        .arg("bestaudio")
        .arg("--get-url")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn yt-dlp for url extraction")?
        .wait_with_output()
        .await
        .context("Failed to wait for yt-dlp")?;
    
    if !output.status.success() {
        anyhow::bail!("yt-dlp get-url failed");
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let url = stdout.trim().to_string();
    log::info!("Extracted direct URL: {}", url);
    Ok(url)
}
