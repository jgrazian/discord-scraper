# Simple Discord Scraper
A simple program to scrape all of the messages from a list of given Discord channels.

## Usage
Example Usage:
```bash
cargo run -- 640173126345367322,540171126342367302 -d "./data/messages.db" -a "MY_DISCORD_AUTH_TOKEN"
```
Alternatively set the `DISCORD_AUTH_TOKEN` env var:
```bash
$Env:DISCORD_AUTH_TOKEN = "MY_DISCORD_AUTH_TOKEN"
cargo run -- 640173126345367322,540171126342367302 -d "./data/messages.db"
```

For help:
``` bash
cargo run -- -h
```
```bash
discord-scraper 0.1.0

USAGE:
    discord-scraper.exe [OPTIONS] [CHANNEL_IDS]...

ARGS:
    <CHANNEL_IDS>...

OPTIONS:
    -a, --auth <AUTH>          Discord authorization token
    -d, --db-path <DB_PATH>    Database path [default: ./data/messages.db]
    -h, --help                 Print help information
    -V, --version              Print version information
```

## Getting the Auth Token
The easiest way to get your Discord authorization token is to do the following:
1. Login to Discord in a web-browser
2. Open Developer Tools (F12)
3. Connect to a channel
4. In the 'Network' tab of the Developer Tools look for a request called `science`
5. Grab the Auth Token from the header field it should look something like 
`Authorization 77D59F918A8728978D62C5776C9ABECBF6CC462D586F4C73750432A118619906`
