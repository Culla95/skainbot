use std::process::{Stdio, Command};
use songbird::input::{Input, RawAdapter, ChildContainer, core::io::ReadOnlySource};
use anyhow::{Result, Context};

pub async fn get_ffmpeg_input(url: String) -> Result<Input> {
    log::info!("Creating ytdl input for url: {}", url);
    let mut child = Command::new("ffmpeg")
        .arg("-y")
        .args(&[
            "-reconnect", "1",
            "-reconnect_streamed", "1",
            "-reconnect_delay_max", "5",
            "-i", &url,
            "-vn",
            "-f", "f32le", 
            "-ac", "2",
            "-ar", "48000",
            "-acodec", "pcm_f32le",
            "-"
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .context("Failed to spawn ffmpeg")?;

    let stderr = child.stderr.take().context("Failed to take stderr")?;
    tokio::spawn(async move {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
             if let Ok(l) = line {
                 log::info!("[ffmpeg] {}", l);
             }
        }
    });

    let container = ChildContainer::from(vec![child]);
    let reader = ReadOnlySource::new(container);
    let adapter = RawAdapter::new(reader, 48000, 2);
    
    Ok(Input::from(adapter))
}





