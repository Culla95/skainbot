use std::process::Stdio;
use std::path::Path;
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

    let mut cmd = Command::new("yt-dlp");
    cmd.arg("--dump-single-json")
        .arg("--no-warnings")
        .arg("--skip-download")
        .arg("--flat-playlist")
        .arg("--extractor-args")
        .arg("youtube:player-client=ios,android,mweb")
        .arg("--force-ipv4")
        .arg("--user-agent")
        .arg("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1")
        .arg(&arg);

    if Path::new("cookies.txt").exists() {
        log::info!("Using cookies.txt for search");
        cmd.arg("--cookies").arg("cookies.txt");
    } else {
        log::warn!("cookies.txt NOT found in current directory!");
    }

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

pub async fn get_direct_url(url: &str) -> Result<String> {
    let mut cmd = Command::new("yt-dlp");
    cmd.arg("-f")
        .arg("bestaudio")
        .arg("--extractor-args")
        .arg("youtube:player-client=ios,android,mweb")
        .arg("--force-ipv4")
        .arg("--user-agent")
        .arg("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1")
        .arg("--get-url")
        .arg(url);

    if Path::new("cookies.txt").exists() {
        log::info!("Using cookies.txt for URL extraction");
        cmd.arg("--cookies").arg("cookies.txt");
    }

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn yt-dlp for url extraction")?
        .wait_with_output()
        .await
        .context("Failed to wait for yt-dlp")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp get-url failed: {}", stderr);
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let url = stdout.trim().to_string();
    log::info!("Extracted direct URL: {}", url);
    Ok(url)
}
