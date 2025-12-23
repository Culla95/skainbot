# Skainbot - Discord Music Bot (Rust)

A high-performance Discord music bot written in Rust, utilizing `serenity`, `songbird`, `yt-dlp`, and `ffmpeg`.

## Prerequisites

1.  **Rust**: Stable toolchain installed (`rustup`).
2.  **External Tools** (Must be in PATH):
    -   `yt-dlp`: For extraction of media URLs and metadata.
    -   `ffmpeg`: For streaming and transcoding audio.

## Installation

1.  Clone the repository.
2.  Install dependencies and build:
    ```bash
    cargo build --release
    ```

## Configuration

Set the `DISCORD_TOKEN` environment variable:

```bash
export DISCORD_TOKEN="your_discord_bot_token"
```

## Running

```bash
cargo run --release
```

## Commands

-   `!join`: Bot joins your voice channel.
-   `!play <url|query>`: Play a song or playlist (searches YouTube if query).
-   `!skip`: Skip current song.
-   `!next <url|query>`: Add a song to the top of the queue.
-   `!queue`: Show current queue.
-   `!clear`: Clear the queue.

## Architecture

-   **Queue**: In-memory `VecDeque` protected by `Mutex`.
-   **Streaming**: Spawns `ffmpeg` with `-re` (real-time) flag to pipe audio to Songbird.
-   **Extraction**: Uses `yt-dlp` to resolve specific media URLs without downloading files to disk.
-   **Discord**: `serenity` for gateway/events, `songbird` for voice.

## License

MIT
