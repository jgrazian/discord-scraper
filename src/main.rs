use clap::Parser;
use reqwest::blocking::Response;
use serde::{Deserialize, Serialize};

use std::env;
use std::error::Error;
use std::io::Read;
use std::path::Path;

const BASE_URL: &str = "https://discord.com/api/v10";

type SimpleResult<T> = Result<T, Box<dyn Error>>;

fn main() -> SimpleResult<()> {
    let mut args = Args::parse();

    if args.auth.is_none() {
        if let Ok(auth) = env::var("DISCORD_AUTH_TOKEN") {
            args.auth = Some(auth);
        } else {
            println!("No authorization token found!");
            std::process::exit(1);
        }
    }

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("authorization", args.auth.unwrap().parse().unwrap());

    let client = reqwest::blocking::Client::builder()
        .user_agent("MessageScraperBot (1.0.0)")
        .default_headers(headers)
        .build()?;

    let db_path = std::path::Path::new(&args.db_path);
    let prefix = db_path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();
    let mut conn = connect_db(db_path)?;

    for channel_id in &args.channel_ids {
        let channel = get_channel(&client, channel_id)?;
        insert_channel(&mut conn, channel)?;

        get_channel_messages(&mut conn, &client, channel_id)?;
    }

    Ok(())
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    /// Discord authorization token
    #[clap(short, long)]
    auth: Option<String>,

    channel_ids: Vec<String>,

    /// Database path
    #[clap(short, long, default_value_t = String::from("./data/messages.db"))]
    db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Channel {
    id: String,
    guild_id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    id: String,
    channel_id: String,
    author: User,
    content: String,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: String,
    username: String,
    discriminator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscordError {
    message: String,
    code: usize,
}

fn connect_db<P: AsRef<Path>>(path: P) -> SimpleResult<rusqlite::Connection> {
    if !path.as_ref().exists() {
        return create_db(path);
    }

    return Ok(rusqlite::Connection::open(path)?);
}

fn create_db<P: AsRef<Path>>(path: P) -> SimpleResult<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;

    conn.execute(
        "CREATE TABLE channel (
                  id              TEXT PRIMARY KEY,
                  guild_id        TEXT,
                  name            TEXT
                  ) STRICT;",
        [],
    )?;
    conn.execute(
        "CREATE TABLE message (
                  id              TEXT PRIMARY KEY,
                  channel_id      TEXT REFERENCES channel(id),
                  author_id       TEXT REFERENCES user(id),
                  content         TEXT NOT NULL,
                  timestamp       TEXT NOT NULL
                  ) STRICT;",
        [],
    )?;
    conn.execute(
        "CREATE TABLE user (
                  id              TEXT PRIMARY KEY,
                  username        TEXT NOT NULL,
                  discriminator   TEXT NOT NULL
                  ) STRICT;",
        [],
    )?;

    return Ok(conn);
}

fn insert_channel(conn: &mut rusqlite::Connection, channel: Channel) -> SimpleResult<()> {
    println!(
        "[INFO] Inserting 1 Channel: {}",
        channel.name.as_ref().unwrap_or(&"".to_string())
    );

    conn.execute(
        "INSERT OR IGNORE INTO channel (id, guild_id, name) VALUES (?,?,?)",
        [
            channel.id,
            channel.guild_id.unwrap_or("".to_string()),
            channel.name.unwrap_or("".to_string()),
        ],
    )?;

    Ok(())
}

fn insert_users(conn: &mut rusqlite::Connection, users: Vec<User>) -> SimpleResult<()> {
    let tx = conn.transaction()?;
    for user in users {
        let mut stmt = tx.prepare("INSERT OR IGNORE INTO user (id, username, discriminator) VALUES (?,?,?) RETURNING username")?;

        let mut rows = stmt.query(rusqlite::params![
            user.id,
            user.username,
            user.discriminator
        ])?;
        while let Some(row) = rows.next()? {
            println!("[INFO] Inserting 1 User: {:?}", row.get::<_, String>(0)?);
        }
    }
    tx.commit()?;

    Ok(())
}

fn insert_messages(conn: &mut rusqlite::Connection, messages: Vec<Message>) -> SimpleResult<()> {
    println!("[INFO] Inserting {} Messages", &messages.len());

    let tx = conn.transaction()?;
    for msg in messages {
        tx.execute(
            "INSERT OR IGNORE INTO message (id, channel_id, author_id, content, timestamp) VALUES (?,?,?,?,?)",
            [
                msg.id,
                msg.channel_id,
                msg.author.id,
                msg.content,
                msg.timestamp
            ],
        )?;
    }
    tx.commit()?;

    Ok(())
}

fn send_request(client: &reqwest::blocking::Client, req_url: &str) -> SimpleResult<Response> {
    const RETRY_PAD: f64 = 0.1;
    let res = client.get(req_url).send()?;

    if res.status() == reqwest::StatusCode::OK {
        return Ok(res);
    }

    if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_time = res
            .headers()
            .get("Retry-After")
            .unwrap()
            .to_str()?
            .parse::<f64>()?;

        println!("[WARN] Too many requests. Sleeping for {}s.", retry_time);

        std::thread::sleep(std::time::Duration::from_secs_f64(retry_time + RETRY_PAD));

        return send_request(client, req_url);
    }

    let err: DiscordError = serde_json::from_str(&res.text()?)?;
    let err_msg = format!("While executing request {}: {}", req_url, err.message);
    return Err(err_msg.into());
}

fn get_messages(
    client: &reqwest::blocking::Client,
    channel_id: &str,
    before: Option<String>,
) -> SimpleResult<Vec<Message>> {
    let req_url = if let Some(before_id) = before {
        format!(
            "{}/channels/{}/messages?limit=100&before={}",
            BASE_URL, channel_id, before_id
        )
    } else {
        format!("{}/channels/{}/messages?limit=100", BASE_URL, channel_id)
    };

    let mut res = send_request(client, &req_url)?;

    let mut body = String::new();
    res.read_to_string(&mut body)?;
    let messages: Vec<Message> = serde_json::from_str(&body)?;
    Ok(messages)
}

fn get_channel_messages(
    conn: &mut rusqlite::Connection,
    client: &reqwest::blocking::Client,
    channel_id: &str,
) -> SimpleResult<()> {
    let mut before = None;
    let mut messages = get_messages(client, channel_id, before)?;

    while !messages.is_empty() {
        let users: Vec<User> = messages.iter().map(|m| m.author.clone()).collect();
        insert_users(conn, users)?;

        before = Some(messages.last().unwrap().id.clone());
        insert_messages(conn, messages)?;

        messages = get_messages(client, channel_id, before)?;
    }

    Ok(())
}

fn get_channel(client: &reqwest::blocking::Client, channel_id: &str) -> SimpleResult<Channel> {
    let req_url = format!("{}/channels/{}", BASE_URL, channel_id);

    let mut res = send_request(client, &req_url)?;

    let mut body = String::new();
    res.read_to_string(&mut body)?;
    let channel: Channel = serde_json::from_str(&body)?;
    Ok(channel)
}
