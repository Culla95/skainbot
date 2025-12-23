use std::env;
use std::sync::Arc;
use std::time::Duration;
use simplelog::*;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::{Ready, GatewayIntents};
use serenity::prelude::*;
use songbird::{SerenityInit, Call}; 

mod queue;
mod yt;
mod stream;

use queue::{QueueManager};

struct QueueManagerKey;
impl TypeMapKey for QueueManagerKey {
    type Value = Arc<QueueManager>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        if !msg.content.starts_with("!") {
            return;
        }

        let ctx = ctx.clone();
        let msg_clone = msg.clone();

        tokio::spawn(async move {
            process_command(ctx, msg_clone).await;
        });
    }
}

async fn process_command(ctx: Context, msg: Message) {
    let content = msg.content.clone();
    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.is_empty() { return; }
    let cmd = parts[0];
    let args = &parts[1..];

    if let Err(e) = msg.delete(&ctx.http).await {
        log::warn!("Failed to delete user message: {:?}", e);
    }

    log::info!("Processing command: {}", cmd);
    match cmd {
        "!join" => handle_join(&ctx, &msg).await,
        "!play" => handle_play(&ctx, &msg, args.join(" ")).await,
        "!skip" => handle_skip(&ctx, &msg).await,
        "!next" => handle_next(&ctx, &msg, args.join(" ")).await,
        "!queue" => handle_queue(&ctx, &msg).await,
        "!nowplaying" => handle_nowplaying(&ctx, &msg).await,
        "!clear" => handle_clear(&ctx, &msg).await,
        _ => {}
    }
}

async fn send_temp_message(ctx: &Context, channel_id: serenity::model::id::ChannelId, text: &str) {
    if let Ok(msg) = channel_id.say(&ctx.http, text).await {
        let http = ctx.http.clone();
        let msg_id = msg.id;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Err(e) = channel_id.delete_message(&http, msg_id).await {
                log::warn!("Failed to delete bot message: {:?}", e);
            }
        });
    }
}

async fn ensure_joined(ctx: &Context, msg: &Message) -> bool {
    let guild_id = msg.guild_id.expect("Guild ID");
    let manager = songbird::get(ctx).await.expect("Songbird").clone();

    if manager.get(guild_id).is_some() {
        return true;
    }

    let channel_id = {
        let guild = msg.guild(&ctx.cache);
        if let Some(guild) = guild {
            guild.voice_states.get(&msg.author.id).and_then(|vs| vs.channel_id)
        } else {
            None
        }
    };

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            send_temp_message(ctx, msg.channel_id, "You need to be in a voice channel first.").await;
            return false;
        }
    };

    if let Ok(_handler_lock) = manager.join(guild_id, connect_to).await {
        send_temp_message(ctx, msg.channel_id, &format!("Joined {}", connect_to.mention())).await;
        true
    } else {
        send_temp_message(ctx, msg.channel_id, "Error joining the channel").await;
        false
    }
}

async fn handle_join(ctx: &Context, msg: &Message) {
    let _ = ensure_joined(ctx, msg).await;
}

async fn handle_play(ctx: &Context, msg: &Message, query: String) {
    if !ensure_joined(ctx, msg).await {
        return;
    }
    if query.trim().is_empty() {
        send_temp_message(ctx, msg.channel_id, "Please provide a URL or search query.").await;
        return;
    }

    send_temp_message(ctx, msg.channel_id, "Searching...").await;

    // Search/Expand
    // Fix: pass msg.author.id (UserId)
    let res = yt::search_query(&query, msg.author.id).await;
    match res {
        Ok(tracks) => {
            if tracks.is_empty() {
                 send_temp_message(ctx, msg.channel_id, "No results found.").await;
                 return;
            }

            let data = ctx.data.read().await;
            let queue_manager = data.get::<QueueManagerKey>().expect("QueueManager").clone();
            
            let count = tracks.len();
            let first_track = tracks[0].clone();
            
            for track in tracks {
                queue_manager.add(track);
            }

            if count == 1 {
                 send_temp_message(ctx, msg.channel_id, &format!("Added to queue: {}", first_track.title)).await;
            } else {
                 send_temp_message(ctx, msg.channel_id, &format!("Added {} tracks to queue.", count)).await;
            }
            
            // Trigger playback check
            check_playback(ctx.clone(), msg.guild_id.unwrap()).await;
        },
        Err(e) => {
            send_temp_message(ctx, msg.channel_id, &format!("Error: {}", e)).await;
        }
    }
}

async fn handle_skip(ctx: &Context, msg: &Message) {
    let manager = songbird::get(ctx).await.expect("Songbird").clone();
    if let Some(handler_lock) = manager.get(msg.guild_id.unwrap()) {
        let handler = handler_lock.lock().await;
        let _ = handler.queue().skip();
        send_temp_message(ctx, msg.channel_id, "Skipped.").await;
    }
    // We rely on the end event or check_playback?
    // If we stop, the end event should fire? 
    // Usually yes.
    // But verify: Songbird stop() clears the current track. 
    // Does it fire `TrackEvent::End`?
    // It should. 
    // Just in case, we can manually check playback *after* a small delay or trust the event.
    // For now, let's trust the event.
}

async fn handle_next(ctx: &Context, msg: &Message, query: String) {
    if query.trim().is_empty() { return; }
    
    if !ensure_joined(ctx, msg).await {
        return;
    }
    
    let res = yt::search_query(&query, msg.author.id).await;
    match res {
        Ok(tracks) => {
             let data = ctx.data.read().await;
             let queue_manager = data.get::<QueueManagerKey>().expect("QueueManager").clone();
             
             for track in tracks.iter().rev() {
                 queue_manager.add_front(track.clone());
             }
             send_temp_message(ctx, msg.channel_id, &format!("Added {} tracks to the top of the queue.", tracks.len())).await;
             
             // Trigger playback check
             check_playback(ctx.clone(), msg.guild_id.unwrap()).await;
        },
        Err(e) => { send_temp_message(ctx, msg.channel_id, &format!("Error: {}", e)).await; }
    }
}

async fn handle_queue(ctx: &Context, msg: &Message) {
    let data = ctx.data.read().await;
    let queue_manager = data.get::<QueueManagerKey>().expect("QueueManager").clone();
    let tracks = queue_manager.list(10);
    
    if tracks.is_empty() {
        send_temp_message(ctx, msg.channel_id, "Queue is empty.").await;
        return;
    }
    
    let mut response = String::from("**Queue:**\n");
    for (i, track) in tracks.iter().enumerate() {
        response.push_str(&format!("{}. {}\n", i + 1, track.title));
    }
    send_temp_message(ctx, msg.channel_id, &response).await;
}

async fn handle_nowplaying(ctx: &Context, msg: &Message) {
    send_temp_message(ctx, msg.channel_id, "Now playing info needs implementation.").await;
}

async fn handle_clear(ctx: &Context, msg: &Message) {
    let data = ctx.data.read().await;
    let queue_manager = data.get::<QueueManagerKey>().expect("QueueManager").clone();
    queue_manager.clear();
    send_temp_message(ctx, msg.channel_id, "Queue cleared.").await;
}

async fn check_playback(ctx: Context, guild_id: serenity::model::id::GuildId) {
    let manager = songbird::get(&ctx).await.expect("Songbird").clone();
    
    let handler_lock = match manager.get(guild_id) {
        Some(h) => h,
        None => return,
    };
    
    let mut handler: tokio::sync::MutexGuard<Call> = handler_lock.lock().await;

    // If already playing, do nothing
    if !handler.queue().is_empty() {
       log::info!("Playback already in progress, skipping manual trigger.");
       return;
    }

    let data = ctx.data.read().await;
    let queue_manager = data.get::<QueueManagerKey>().expect("QueueManager").clone();
    
    // Loop until we find a playable track or queue empty
    while let Some(track) = queue_manager.pop() {
        log::info!("Attempting to play track: {}", track.title);
        let stream_url = match yt::get_direct_url(&track.url).await {
            Ok(u) => {
                log::info!("Got direct URL for {}", track.title);
                u
            },
            Err(e) => {
                log::error!("Failed to get valid URL for {}: {}", track.title, e);
                continue; // Try next
            }
        };

        let input = match stream::get_ffmpeg_input(stream_url).await {
            Ok(i) => {
                log::info!("Created input for {}", track.title);
                i
            },
            Err(e) => {
                log::error!("Failed to create input for {}: {}", track.title, e);
                continue;
            }
        };
        
        log::info!("Enqueuing input for {}", track.title);
        let track_handle = handler.enqueue_input(input).await;
        
        // Add event listener
        let _ = track_handle.add_event(
            songbird::Event::Track(songbird::TrackEvent::End),
            TrackEndNotifier {
                ctx: ctx.clone(),
                guild_id,
            },
        );
        log::info!("Playback started (enqueued) for {}", track.title);
        
        break; // Successfully started playing one track
    }
}


struct TrackEndNotifier {
    ctx: Context,
    guild_id: serenity::model::id::GuildId,
}

#[async_trait]
impl songbird::EventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        log::info!("Track ended.");
        check_playback(self.ctx.clone(), self.guild_id).await;
        None
    }
}

#[tokio::main]
async fn main() {
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    #[allow(deprecated)]
    let framework = serenity::framework::StandardFramework::new(); // Warn is fine

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .framework(framework)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<QueueManagerKey>(Arc::new(QueueManager::new()));
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

