extern crate dirs;
extern crate reqwest;
extern crate crossterm_input;
extern crate crossterm;
extern crate serde;
extern crate clipboard;
extern crate roxmltree;
extern crate chrono;
extern crate ctrlc;
extern crate base64;
extern crate html2text;
extern crate percent_encoding;
extern crate blockish;
extern crate blockish_player;

use std::io;
use utf8::BufReadDecoder;
use std::fs::File;
use std::time::Instant;
use clipboard::{ClipboardProvider, ClipboardContext};
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use std::io::{Read, Write, BufReader};
use std::io::ErrorKind::NotFound;
use std::cmp::min;
use std::process::{Command, Stdio};
use crossterm_input::{input, RawScreen, InputEvent, MouseEvent, MouseButton};
use crossterm_input::KeyEvent::{Char, Down, Up, Left, Right, Ctrl};
use futures::future::join_all;
use chrono::DateTime;
use regex::Regex;
use reqwest::header::{HeaderValue, HeaderMap, ETAG, IF_NONE_MATCH, ACCEPT_ENCODING};
use std::collections::HashMap;
use percent_encoding::percent_decode;
use blockish::render_image;

use webbrowser;

#[derive(Debug)]
enum CustomError {
    Io(std::io::Error),
    Reqwest(reqwest::Error),
}

impl From<std::io::Error> for CustomError {
    fn from(err: std::io::Error) -> CustomError {
        CustomError::Io(err)
    }
}

impl From<reqwest::Error> for CustomError {
    fn from(err: reqwest::Error) -> CustomError {
        CustomError::Reqwest(err)
    }
}

fn default_content() -> Option<String> {
    None
}

fn default_kind_symbols() -> HashMap<String, String> {
    let mut symbols : HashMap<String, String> = HashMap::new();
    symbols.insert("Audio".to_string(), "a".to_string());
    symbols.insert("Video".to_string(), "v".to_string());
    symbols.insert("Other".to_string(), "o".to_string());
    symbols.insert("Magnet".to_string(), "m".to_string());
    symbols
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
struct AppConfig {
    video_path: String,
    cache_path: String,
    youtubedl_format: String,
    video_extension: String,
    kind_symbols: HashMap<String, String>,
    blockish_player: Option<String>,
    players: Vec<Vec<String>>,
    channel_ids: Vec<String>,
    channel_urls: Vec<String>,
    mpv_mode: bool,
    mpv_path: String,
    fs: bool,
    open_magnet: Option<String>,
    sort: String,
}

impl Default for AppConfig {
    fn default() -> AppConfig {
        AppConfig {
            kind_symbols: default_kind_symbols(),
            video_path: "/tmp".to_string(),
            cache_path: "__HOME/.cache/yts/yts.json".to_string(),
            youtubedl_format: "[height <=? 360][ext = mp4]".to_string(),
            video_extension: "mp4".to_string(),
            blockish_player: None,
            players: vec![
                vec!["/usr/bin/omxplayer".to_string(), "-o".to_string(), "local".to_string()],
                vec!["/Applications/VLC.app/Contents/MacOS/VLC".to_string(), "--play-and-exit".to_string(), "-f".to_string()],
                vec!["/usr/bin/vlc".to_string(), "--play-and-exit".to_string(), "-f".to_string()],
                vec!["/usr/bin/mpv".to_string(), "-really-quiet".to_string(), "-fs".to_string()],
                vec!["/usr/bin/mplayer".to_string(), "-really-quiet".to_string(), "-fs".to_string()],
            ],
            channel_ids: vec![],
            channel_urls: vec![],
            mpv_mode: true,
            mpv_path: "/usr/bin/mpv".to_string(),
            fs: true,
            open_magnet: None,
            sort: "desc".to_string(),
        }
    }
}

fn load_config() -> Result<AppConfig, std::io::Error> {
    match dirs::home_dir() {
        Some(home) => {
            match home.to_str() {
                Some(h) => {
                    let path = format!("{}/.config/youtube-subscriptions/config.json",
                                       h);
                    let s = fs::read_to_string(path)?;
                    let mut _res = serde_json::from_str::<AppConfig>(s.as_str())?;
                    _res.video_path = _res.video_path.replace("__HOME", &h);
                    fs::create_dir_all(&_res.video_path)?;
                    _res.cache_path = _res.cache_path.replace("__HOME", &h);
                    match Path::new(&_res.cache_path).parent() {
                        Some(dirname) => fs::create_dir_all(&dirname)?,
                        None => {
                            debug(&format!("failed to find dirname of {}", &_res.cache_path));
                        }
                    }
                    Ok(_res)
                },
                None => Ok(AppConfig { ..Default::default() })
            }
        },
        None =>
            Ok(AppConfig { ..Default::default() })
    }
}

fn load_config_fallback() -> AppConfig {
    match load_config() {
        Ok(res) => res,
        Err(e) => {
            debug(&format!("load_config err: {}", e));
            AppConfig { ..Default::default() }
        }
    }
}

fn subscriptions_url() -> &'static str {
    "https://www.youtube.com/subscription_manager?action_takeout=1"
}

fn download_subscriptions() {
    let _res = webbrowser::open(&subscriptions_url());
    debug(&format!("please save file to ~/{}", subscription_manager_relative_path()));
}

fn subscription_manager_relative_path() -> &'static str {
    ".config/youtube-subscriptions/subscription_manager"
}

fn get_subscriptions_xml() -> Result<String, std::io::Error> {
    match dirs::home_dir() {
        Some(home) =>
            match home.to_str() {
                Some(s) => {
                    let path = format!("{}/{}", s, subscription_manager_relative_path());
                    if fs::metadata(&path).is_ok() {
                        fs::read_to_string(path)
                    }
                    else {
                        Ok("<opml></opml>".to_string())
                    }
                },
                None => {
                    debug("failed conversting home to str");
                    Ok("<opml></opml>".to_string())
                }
            },
        None => {
            debug("failed finding home dir");
            Ok("<opml></opml>".to_string())
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum Flag {
    Read,
}

fn flag_to_string(flag: &Option<Flag>) -> String {
    match flag {
        Some(Flag::Read) => "✓".to_string(),
        None => " ".to_string(),
    }
}

fn default_thumbnail() -> String {
    "".to_string()
}

fn default_flag() -> Option<Flag> {
    None
}

fn kind_symbol(app_config: &AppConfig, kind: &ItemKind) -> String {
    match app_config.kind_symbols.get(&format!("{:?}", kind).to_string()) {
        Some(symbol) => symbol.to_string(),
        _ => " ".to_string(),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum ItemKind {
    Video,
    Audio,
    Other,
    Magnet,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Item {
    kind: ItemKind,
    channel_url: String,
    channel: String,
    title: String,
    url: String,
    published: String,
    description: String,
    #[serde(default = "default_thumbnail")]
    thumbnail: String,
    #[serde(default = "default_flag")]
    flag: Option<Flag>,
    #[serde(default = "default_content")]
    content: Option<String>,
}

type ChannelEtags = HashMap<String, Option<String>>;

#[derive(Serialize, Deserialize, Debug)]
struct Items {
    channel_etags: ChannelEtags,
    videos: Vec<Item>,
}

struct ChanelItems {
    channel_url: String,
    etag: Option<String>,
    videos: Vec<Item>,
}

macro_rules! get_decendant_node {
    ( $node:expr, $name:expr  ) => {
        $node.descendants().find(|n| n.tag_name().name() == $name).unwrap_or($node)
    }
}

fn get_rss_videos(document: roxmltree::Document, channel_url: &String) -> Vec<Item> {
    let title = match document.descendants().find(|n| n.tag_name().name() == "title") {
        Some(node) => node.text().unwrap_or(""),
        None => {
            debug("did not find title node");
            ""
        }
    };
    document.descendants().filter(|n| n.tag_name().name() == "entry").map(|entry| {
        let mut kind = ItemKind::Other;
        let url = get_decendant_node!(entry, "link").attribute("href").unwrap_or("");
        let video_title = get_decendant_node!(entry, "title").text().unwrap_or("");
        let video_published = get_decendant_node!(entry, "published").text().unwrap_or(
            get_decendant_node!(entry, "updated").text().unwrap_or(
                    get_decendant_node!(entry, "issued").text().unwrap_or("")
                )
            );
        let thumbnail = get_decendant_node!(entry, "thumbnail").attribute("url").unwrap_or("");
        if thumbnail != "".to_string() { kind = ItemKind::Video } 
        let group = get_decendant_node!(entry, "group");
        let description = match get_decendant_node!(group, "description").text() {
            Some(stuff) => stuff,
            None => "",
        };
        let content = get_decendant_node!(group, "content").text().map(|x| x.to_string());
        Item { 
            kind: kind,
            channel: title.to_string(),
            title: video_title.to_string(),
            url: url.to_string(),
            published: video_published.to_string(),
            description: description.to_string(),
            thumbnail: thumbnail.to_string(),
            flag: default_flag(),
            content: content,
            channel_url: channel_url.to_string()
        }
    }).collect::<Vec<Item>>()
}

fn get_atom_videos(channel: roxmltree::Node, channel_url: &String) -> Vec<Item> {
    let title = get_decendant_node!(channel, "title").text().unwrap_or("");
    channel.descendants().filter(|n| n.tag_name().name() == "item").map(|entry| {
        let mut kind = ItemKind::Other;
        let url = get_decendant_node!(entry, "enclosure").attribute("url").map( |x| {
            kind = if x.starts_with("magnet:") {
                ItemKind::Magnet
            }
            else {
                ItemKind::Audio
            };
            x
        }).unwrap_or(
            get_decendant_node!(entry, "link").text().unwrap_or("")
        );
        let video_title = get_decendant_node!(entry, "title").text().unwrap_or("");
        let video_published = get_decendant_node!(entry, "pubDate").text().unwrap_or("");
        let thumbnail = get_decendant_node!(entry, "thumbnail").attribute("url").unwrap_or("");
        if thumbnail != "".to_string() { kind = ItemKind::Video; }
        let description = get_decendant_node!(entry, "description").text().unwrap_or("");
        let date = match DateTime::parse_from_rfc2822(video_published) {
            Ok(x) => x.to_rfc3339(),
            Err(_) => chrono::offset::Local::now().to_rfc3339(),
        };
        let content = get_decendant_node!(entry, "encoded").text().map(|x| x.to_string());
        Item { 
            kind: kind,
            channel: title.to_string(),
            title: video_title.to_string(),
            url: url.to_string(),
            published: date,
            description: description.to_string(),
            thumbnail: thumbnail.to_string(),
            content: content,
            flag: default_flag(),
            channel_url: channel_url.to_string()
        }
    }).collect::<Vec<Item>>()
}

fn get_channel_videos_from_contents(contents: &str, channel_url: &String) -> Vec<Item> {
    match roxmltree::Document::parse(contents) {
        Ok(document) =>
            match document.descendants().find(|n| n.tag_name().name() == "channel") {
                Some(channel) => get_atom_videos(channel, &channel_url),
                None => get_rss_videos(document, &channel_url),
            },
        Err(e) => {
            debug(&format!("failed parsing xml {}", e));
            vec![]
        },
    }
}

fn get_original_channel_videos(channel_url: &String, channel_etag: &Option<&String>, original_videos: &Items) -> Option<ChanelItems> {
    let mut channel_videos: Vec<Item> = vec![];
    for video in original_videos.videos.iter() {
        if &video.channel_url == channel_url {
            channel_videos.push(video.clone())
        }
    };
    Some(ChanelItems {
                        channel_url: channel_url.to_string(),
                        etag: channel_etag.map(|x| x.to_string()),
                        videos: channel_videos,
                    })
}

fn get_headers(channel_etag: Option<&String>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("*/*"));
    match channel_etag {
        Some(etag) => 
            match HeaderValue::from_str(etag.as_str()) {
                Ok(s) => {
                    headers.insert(IF_NONE_MATCH, s);
                },
                _ => {}
            },
        _ => {}
    }
    headers
}

#[derive(Debug)]
struct ChannelURLWithBasicAuth {
    channel_url: String,
    password: Option<String>,
    user: Option<String>,
}

fn parse_basic_auth(channel_url: &String) -> ChannelURLWithBasicAuth {
    match Regex::new(r"^(https://)([^:/]*):([^@/]*)@(.*)$") {
        Ok(re) => 
            match re.captures(channel_url) {
                Some(caps) => ChannelURLWithBasicAuth { 
                    channel_url: (format!("{}{}", caps[1].to_string(), caps[4].to_string())).to_string(), 
                    password: Some(percent_decode(caps[3].as_bytes()).decode_utf8_lossy().to_string()),
                    user: Some(percent_decode(caps[2].as_bytes()).decode_utf8_lossy().to_string()) },
                None => ChannelURLWithBasicAuth {
                    channel_url: channel_url.to_string(), password: None, user: None },
            }
        Err(_) => ChannelURLWithBasicAuth { channel_url: channel_url.to_string(), password: None, user: None },
    }
}

fn build_request(channel_url: &String, client: &reqwest::Client, channel_etag: Option<&String>) -> reqwest::RequestBuilder {
    let channel_url_with_basic_auth = parse_basic_auth(&channel_url);
    match channel_url_with_basic_auth.user {
        Some(user) => 
            client.get(channel_url_with_basic_auth.channel_url.as_str())
            .headers(get_headers(channel_etag)).basic_auth(user, channel_url_with_basic_auth.password)
            ,
        None => client.get(channel_url_with_basic_auth.channel_url.as_str())
            .headers(get_headers(channel_etag)),
    }
}

async fn get_channel_videos(client: &reqwest::Client, channel_url: String, channel_etag: Option<&String>, original_videos: &Items) -> Option<ChanelItems> {
    for _i in 0..2 {
        let request = build_request(&channel_url, &client, channel_etag);
        let wrapped_response = request.send().await;
        match wrapped_response {
            Ok(response) => {
                let status = response.status();
                if status.as_u16() == 304 {
                    return get_original_channel_videos(&channel_url, &channel_etag, &original_videos);
                }
                else if status.is_success() {
                    let headers = response.headers();
                    let etag_opt_opt = headers.get(ETAG).map (|x| x.to_str().ok().map(|y| y.to_string()));
                    match response.text().await {
                        Ok(text) => { 
                            return Some(
                            ChanelItems {
                                channel_url: channel_url.to_string(),
                                etag: match etag_opt_opt { Some(Some(x)) => Some(x), _ => None },
                                videos: get_channel_videos_from_contents(&text, &channel_url)
                            })},
                        Err(_) => { }
                    }
                }
            },
            Err(_e) if _i == 1 => debug(&format!("failed loading {}: {}", &channel_url, _e)),
            Err(_) => {
            }
        }
    }
    return None
}

async fn get_videos(xml: String, additional_channel_ids: &[String], additional_channel_urls: &[String], original_videos: &Items) -> Vec<Option<ChanelItems>> {
    match roxmltree::Document::parse(xml.as_str()) {
        Ok(document) => {
            let mut urls_from_xml : Vec<String> = document.descendants().filter(
                |n| n.tag_name().name() == "outline").map(|entry| { entry.attribute("xmlUrl") }).filter_map(|x| x).map(|x| x.to_string()).collect::<Vec<String>>();
            let urls_from_additional = additional_channel_ids.iter().map( |id| "https://www.youtube.com/feeds/videos.xml?channel_id=".to_string() + id);
            let urls_from_additional_2 = additional_channel_urls.iter().map( |url| url.to_string() );
            urls_from_xml.extend(urls_from_additional);
            urls_from_xml.extend(urls_from_additional_2);
            match reqwest::Client::builder().use_rustls_tls().build() {
                Ok(client) => {
                    let futs : Vec<_> = urls_from_xml.iter().map(|url| {
                        let etag = match original_videos.channel_etags.get(&url.to_string()) {
                            Some(Some(string)) => Some(string),
                            _ => None
                        };
                        get_channel_videos(&client, url.to_string(), etag, &original_videos)
                    }).collect();
                    join_all(futs).await
                },
                Err(e) => {
                    debug(&format!("failed instantiating client {}", e));
                    vec![None]
                }
            }
        }
        Err(e) => {
            debug(&format!("failed parsing xml {}", e));
            vec![None]
        }
    }
}

fn to_show_videos(app_config: &AppConfig, videos: &mut Vec<Item>, start: usize, end: usize, filter: &Regex) -> Vec<Item> {
    videos.sort_by(|a, b| b.published.cmp(&a.published));
    let filtered_videos = videos.iter().filter(|video| 
        filter.is_match(&video.title) || filter.is_match(&video.channel) || filter.is_match(&format!("{:?}", video.kind))
    ).cloned().collect::<Vec<Item>>();
    let new_end = std::cmp::min(end, filtered_videos.len());
    let mut result = filtered_videos[start..new_end].to_vec();
    if app_config.sort == "desc" { result.reverse() }
    result
}

fn save_videos(app_config: &AppConfig, videos: &Items) {
    let path = app_config.cache_path.as_str();
    match serde_json::to_string(&videos) {
        Ok(serialized) => {
            match fs::write(&path, serialized) {
                Ok(_) => {},
                Err(e) => {
                    debug(&format!("failed writing {} {}", &path, e));
                }
            }
        },
        Err(e) => {
            debug(&format!("failed serializing videos {}", e));
        }
    }
}

async fn load(reload: bool, app_config: &AppConfig, original_videos: &Items) -> Option<Items> {
    match get_subscriptions_xml() {
        Ok(xml) => {
            let path = app_config.cache_path.as_str();
            if reload || fs::metadata(path).is_err() {
                let mut one_query_failed = false;
                let empty_vec = vec![];
                let mut etags : ChannelEtags = HashMap::new();
                let vids = get_videos(xml, &app_config.channel_ids, &app_config.channel_urls, &original_videos).await
                    .iter().map(|x| 
                        match x.as_ref() {
                            Some(res) => { 
                                etags.insert(res.channel_url.clone(), res.etag.clone());
                                &res.videos
                            },
                            None => {
                                one_query_failed = true;
                                &empty_vec
                            }
                        }
                        ).flat_map(|x| x).cloned().collect::<Vec<Item>>();
                if one_query_failed {
                    return None
                }
                let mut videos = Items {
                    channel_etags: etags,
                    videos:  vids
                };
                
                for vid in videos.videos.iter_mut() {
                    for original_vid in original_videos.videos.iter() {
                        if vid.url == original_vid.url {
                            vid.flag = original_vid.flag.clone();
                        }
                    }
                }
                save_videos(app_config, &videos);
                Some(videos)
            } else {
                match fs::read_to_string(path) {
                    Ok(s) => 
                        match serde_json::from_str(s.as_str()) {
                            Ok(res) => Some(res),
                            Err(e) => {
                                debug(&format!("failed reading {} {}", path, e));
                                None
                            }
                        }
                    Err(_) =>
                        None
                }
            }
        },
        Err(_) =>
            None
    }
}


fn get_lines() -> usize {
    let size = crossterm::terminal::size();
    if let Ok((_, h)) = size {
        (h - 1) as usize
    } else {
        20
    }
}

fn get_cols() -> usize {
    let size = crossterm::terminal::size();
    if let Ok((w, _)) = size {
        w as usize
    } else {
        20
    }
}

fn flush_stdout() {
    match io::stdout().flush() {
        Ok(_) => {},
        Err(_) => {},
    }
}

fn hide_cursor() {
    print!("\x1b[?25l");
    flush_stdout();
}

fn smcup() {
    print!("\x1b[?1049h");
    flush_stdout();
}

fn rmcup() {
    print!("\x1b[?1049l");
    flush_stdout();
}

fn clear() {
    print!("\x1b[2J");
    flush_stdout();
    move_cursor(0);
}

fn show_cursor() {
    print!("\x1b[?25h");
    flush_stdout();
}

fn move_cursor(i: usize) {
    print!("\x1b[{};0f", i + 1);
    flush_stdout();
}

fn move_to_bottom() {
    print!("\x1b[{};0f", get_lines() + 1);
    flush_stdout();
}

fn clear_to_end_of_line() {
    print!("\x1b[K");
    flush_stdout();
}

fn debug(s: &str) {
    move_to_bottom();
    clear_to_end_of_line();
    move_to_bottom();
    print!("{}", s);
    flush_stdout();
}

fn print_selector(i: usize) {
    move_cursor(i);
    print!("\x1b[1m|\x1b[0m\r");
    flush_stdout();
}

fn clear_selector(i: usize) {
    move_cursor(i);
    print!(" ");
    flush_stdout();
}

fn jump(i: usize, new_i: usize) -> usize {
    clear_selector(i);
    new_i
}

fn pause() {
    let input = input();
    let _screen = RawScreen::into_raw_mode();
    let _c = input.read_char();
}

struct YoutubeSubscribtions {
    n: usize,
    start: usize,
    search: Regex,
    filter: Regex,
    i: usize,
    toshow: Vec<Item>,
    videos: Items,
    app_config: AppConfig,
}

fn print_videos(app_config: &AppConfig, toshow: &[Item]) {
    let cols = get_cols();
    let channel_max_size = cols / 3;
    let max = toshow.iter().fold(0, |acc, x| std::cmp::max(std::cmp::min(x.channel.chars().count(), channel_max_size), acc));
    for video in toshow {
        let published = video.published.split('T').collect::<Vec<&str>>();
        let whitespaces = " ".repeat(max - std::cmp::min(video.channel.chars().count(), channel_max_size));
        let channel_short = video.channel.chars().take(channel_max_size).collect::<String>();
        let published_short = if published.len() > 0 && published[0].len() >= 10 {
            published[0][5..10].to_string()
        }
        else {
            "?? ??".to_string()
        };
        let s = format!(" {} {} \x1b[36m{}\x1b[0m \x1b[34m{}\x1b[0m{} {}",  
            flag_to_string(&video.flag),
            kind_symbol(&app_config, &video.kind),
            published_short, channel_short, whitespaces,
            video.title
            );
        println!("{}", s.chars().take(min(s.chars().count(), cols-4+9+9+1)).collect::<String>());
    }
}

fn print_press_any_key_and_pause() {
    println!("press any key to continue...");
    pause();
}

fn read_command_output(command: &mut Command, binary: &str) {
    match command.stdout(Stdio::piped())
        .spawn() {
            Ok(mut child) => {
                let stdout_option = child.stdout.take();
                match stdout_option {
                    Some(stdout) => 
                    {
                        let mut decoder = BufReadDecoder::new(BufReader::new(stdout));
                        loop {
                            let read_result = decoder.next_strict();
                            match read_result {
                                Some(Ok(s)) => {
                                        print!("{}", s);
                                },
                                Some(Err(_)) => { }
                                None => { break; }
                            }
                        }
                    }
                    None => debug("no stdout")
                }
                let stderr_option = child.stderr.take();
                match stderr_option {
                    Some(stderr) => 
                        for byte_result in stderr.bytes() {
                            match byte_result {
                                Ok(byte) => {
                                    print!("{}", byte as char);
                                    flush_stdout();
                                },
                                Err(_) => { }
                            }
                        },
                    None => debug("no stderr")
                }
                match child.wait() {
                    Ok(status) => { 
                        if !status.success() {
                            println!("error while running {:?}, return status: {:?}", command, status.code());
                            print_press_any_key_and_pause()
                        }
                    },
                    Err(e) => {
                        println!("error while running {:?}, error: {:?}", command, e);
                        print_press_any_key_and_pause()
                    }
                }
            },
            Err(e) => {
                if let NotFound = e.kind() {
                    println!("`{}` was not found: maybe you should install it ?", binary)
                } else {
                    println!("error while runnnig {} : {}", binary, e);
                }
                print_press_any_key_and_pause()
            }
        }
}

fn play_video_usual(path: &str, app_config: &AppConfig) {
    for player in &app_config.players {
        if fs::metadata(&player[0]).is_ok() {
            let mut child1 = Command::new(&player[0]);
            for arg in player.iter().skip(1) {
                child1.arg(&arg);
            } 
            read_command_output(child1.arg(path), &player[0]);
            return
        }
    }
}

fn play_video(path: &str, app_config: &AppConfig) {
    match &app_config.blockish_player {
        None => play_video_usual(&path, &app_config),
        Some(player) => {
            match blockish_player::video_command(player, &path.to_string()) {
                Ok(mut command) => {
                    read_command_output(&mut command, &"blockish_player".to_string());
                }
                Err(e) => {
                    debug(&format!("error: {:?}", e));
                    play_video_usual(&path, &app_config);
                }
            };
        }
    }
}

fn download_video(path: &str, id: &str, app_config: &AppConfig) {
    if fs::metadata(&path).is_err() {
        read_command_output(Command::new("youtube-dl")
            .arg("-f")
            .arg(&app_config.youtubedl_format)
            .arg("-o")
            .arg(&path)
            .arg("--")
            .arg(&id), &"youtube-dl".to_string())
    }
}

fn open_magnet(url: &str, app_config: &AppConfig) {
    match &app_config.open_magnet {
        Some(open_magnet) =>
            read_command_output(Command::new(open_magnet)
                .arg(&url), &open_magnet),
        None => {},
    }
}

fn play_url(url: &String, kind: &ItemKind, app_config: &AppConfig) {
    if app_config.mpv_mode && fs::metadata(&app_config.mpv_path).is_ok() {
        let message = format!("playing {} with mpv...", url);
        debug(&message);
            match Command::new(&app_config.mpv_path)
            .arg(if app_config.fs { "-fs" } else { "" })
            .arg("-really-quiet")
            .arg("--ytdl-format=")
            .arg(&app_config.youtubedl_format)
            .arg(&url).spawn() {
                Ok(mut child) => {
                    match child.wait() {
                        Ok(_) => {},
                        Err(e) => { debug(&format!("{}", e)); }
                    }
                },
                _ => {}
            };
    } else {
        clear();
        match kind {
            ItemKind::Audio => {
                play_video(&url, app_config);
            },
            ItemKind::Magnet => {
                open_magnet(&url, app_config);
            },
            _ => {
                let path = format!("{}/{}.{}", app_config.video_path, base64::encode(&url), app_config.video_extension);
                download_video(&path, &url, app_config);
                play_video(&path, app_config);
            }
        }
    }
}

fn play(v: &Item, app_config: &AppConfig) {
    play_url(&v.url, &v.kind, app_config);
}

fn print_help() {
    println!("\x1b[34;1m{}\x1b[0m {}", "youtube-subscriptions", env!("CARGO_PKG_VERSION"));
    println!("\x1b[36m{}\x1b[0m", "a tool to view your video subscriptions in a terminal");
    println!("
  q          quit
  j,l,down   move down
  k,up       move up
  g,H        go to top
  G,L        go to bottom
  M          go to middle
  r,$,left   soft refresh
  P          previous page
  N          next page
  R          full refresh (fetches video list)
  h,?        prints this help
  i,right    prints video information
  /          search
  f          filter
  p,enter    plays selected video
  o          open selected video in browser
  t          tag untag a video as read
  T          display thumbnail in browser
  y          copy video url in system clipboard
  c          download subscriptions default browser
  ")
}

fn split_cols(string: &str, cols: usize) -> Vec<String> {
    let mut chars = string.chars();
    (0..).map(|_| chars.by_ref().take(cols).collect::<String>())
        .take_while(|s| !s.is_empty())
        .collect::<Vec<_>>()

}

fn info_lines(v: &Item) -> Vec<String> {
    let cols = get_cols();
    let mut lines : Vec<String> = vec![];
    lines.push(format!("\x1b[34;1m{}\x1b[0m", v.title));
    lines.push("".to_string());
    lines.push(format!("from \x1b[36m{}\x1b[0m", v.channel));
    lines.push("".to_string());
    v.description.split("\n").for_each( |x| split_cols(&x, cols).iter().for_each( |y| lines.push(y.to_string()) )  );
    match &v.content {
        Some(x) => {
            lines.push("".to_string());
            html2text::from_read(x.as_bytes(), cols).split("\n").for_each( |x| lines.push(x.to_string()));
        },
        None => {}
    }
    lines
}

fn print_lines(lines: &Vec<String>, start: usize, rows: usize) {
    let stop = min(lines.len(), start + rows);
    for (_, line) in lines[start..stop].iter().enumerate() {
        println!("{}", line);
    };
    if stop >= start {
        for _ in (stop - start)..rows {
            println!("\x1b[34;1m~\x1b[0m");
        }
    }
}

/* for this to work, each line should not be greater than the number of cols and there should not
 * be any line feed */
fn less(lines: Vec<String>) {
    let rows = get_lines();
    let delta = rows / 2;
    let mut i = 0;
    loop {
        clear();
        print_lines(&lines, i, rows);
        let input = input();
        let result;
        {
            let _screen = RawScreen::into_raw_mode();
            let mut stdin = input.read_sync();
            result = stdin.next();
        }
        match result {
            None => (),
            Some(key_event) => {
                match key_event {
                    InputEvent::Keyboard(event) => {
                        match event {
                            Char('q')| Left  => {
                                break;
                            },
                            Char('g') => {
                                i = 0;
                            },
                            Char('G') => {
                                i = lines.len() - 1;
                            },
                            Char('k') | Up => {
                                if i > 0 { i = i - 1 };
                            },
                            Char('j') | Down => {
                                if i < lines.len() { i = i + 1 };
                            },
                            Ctrl('u') => {
                                if i > delta { i = i - delta }
                                else { i = 0 };
                            },
                            Ctrl('d') => {
                                if i + delta < lines.len() { i = i + delta }
                                else { i = lines.len() };
                            },
                            _ => {
                            },
                        }
                    },
                    _ => ()
                }
            }
        }
    };
}

fn print_info(v: &Item) {
    less(info_lines(v));
}

fn quit() {
    show_cursor();
    rmcup();
}

impl YoutubeSubscribtions {

    fn clear_and_print_videos(&mut self) {
        clear();
        print_videos(&self.app_config, &self.toshow)
    }

    fn move_page(&mut self, direction: i8) {
        self.n = get_lines();
        if direction == 1 {
            if self.start + 2 * self.n < self.videos.videos.len() {
                self.start += self.n;
            }
        }
        else if direction == 0 {
            self.start = 0;
        }
        else if direction == -1 {
            if self.n > self.start {
                self.start = 0;
            }
            else {
                self.start -= self.n;
            }
        }
        self.toshow = to_show_videos(&self.app_config, &mut self.videos.videos, self.start, self.start + self.n, &self.filter);
        self.i = 0;
        self.clear_and_print_videos()
    }

    fn next_page(&mut self) {
        self.move_page(-1);
    }

    fn previous_page(&mut self) {
        self.move_page(1);
    }

    fn soft_reload(&mut self) {
        self.move_page(0);
    }

    async fn hard_reload(&mut self) {
        let now = Instant::now();
        debug(&"updating video list...".to_string());
        match load(true, &self.app_config, &self.videos).await
        {
            Some(videos) => self.videos = videos,
            None =>  debug("could not load videos"),
        }
        debug(&"".to_string());
        self.soft_reload();
        debug(&format!("reload took {} ms", now.elapsed().as_millis()).to_string());
    }

    fn first_page(&mut self) {
        self.n = get_lines();
        self.toshow = to_show_videos(&self.app_config, &mut self.videos.videos, self.start, self.n, &self.filter);
    }

    fn play_current(&mut self) {
        if self.i < self.toshow.len() {
            play(&self.toshow[self.i], &self.app_config);
            self.flag(&Some(Flag::Read));
            self.clear_and_print_videos();
        }
    }


    async fn display_current_thumbnail(&mut self) -> Result<(), CustomError> {
        if self.i < self.toshow.len() {
            let url = &self.toshow[self.i].thumbnail;
            let resp = reqwest::get(url).await?;
            let path = format!("{}/{}.{}", self.app_config.video_path, base64::encode(&url), ".jpg");
            let mut out = File::create(&path)?;
            let bytes = resp.bytes().await?;
            out.write_all(&bytes[..])?;
            let width = get_cols() as u32 * 8;
            render_image(&path, width);
        }
        Ok(())
    }

    fn open_current(&mut self) {
        if self.i < self.toshow.len() {
            let url = &self.toshow[self.i].url;
            debug(&format!("opening {}", &url));
            let _res = webbrowser::open(&url);
            self.flag(&Some(Flag::Read));
            self.clear_and_print_videos();
        }
    }


    fn find_next(&mut self) -> usize {
        for (i, video) in self.toshow.iter().enumerate() {
            if i > self.i {
                if self.search.is_match(&video.title) || self.search.is_match(&video.channel) {
                    return i;
                }
            }
        }
        self.i 
    }

    fn input_with_prefix(&mut self, start_symbol: &str) -> String {
        move_to_bottom();
        clear_to_end_of_line();
        print!("{}", start_symbol);
        flush_stdout();
        let input = input();
        input.read_line().unwrap_or("".to_string())
    }

    fn search_next(&mut self) {
        clear_selector(self.i);
        self.i = self.find_next();
    }

    fn search(&mut self) {
        let s = self.input_with_prefix("/");
        match Regex::new(&format!(".*(?i){}.*", s)) {
            Ok(regex) => {
                self.search = regex;
                self.i = self.find_next();
            },
            Err(_) => {
                debug("failing creating regex")
            }
        }
        self.clear_and_print_videos()
    }

    fn filter(&mut self) {
        let s = self.input_with_prefix("|");
        match Regex::new(&format!(".*(?i){}.*", s)) {
            Ok(regex) => {
                self.filter = regex;
                self.move_page(0);
            },
            Err(_) => {
                debug("failing creating regex")
            }
        }
        self.clear_and_print_videos()
    }

    fn command(&mut self) {
        let s = self.input_with_prefix(":");
        let s = s.split_whitespace().collect::<Vec<&str>>();
        hide_cursor();
        clear();
        if s.len() == 2 {
            if let "o" = s[0] { play_url(&s[1].to_string(), &ItemKind::Video, &self.app_config) }
        }
        self.clear_and_print_videos()
    }

    fn yank_video_uri(&mut self) {
        let url = &self.toshow[self.i].url;
        match ClipboardProvider::new() {
            Ok::<ClipboardContext, _>(mut ctx) => { 
                match ctx.set_contents(url.to_string()) {
                    Ok(_) => debug(&format!("yanked {}", url)),
                    Err(e) => debug(&format!("failed yanking {}: {}", url, e))
                }
            },
            Err(e) => debug(&format!("error: {:?}", e)),
        }
    }

    fn wait_key_press_and_clear_and_print_videos(&mut self) {
        pause();
        self.clear_and_print_videos()
    }

    fn info(&mut self) {
        if self.i < self.toshow.len() {
            clear();
            print_info(&self.toshow[self.i]);
            self.clear_and_print_videos()
        }
    }

    fn flag(&mut self, flag: &Option<Flag>) {
        if self.i < self.toshow.len() {
            self.toshow[self.i].flag = flag.clone();
            for vid in self.videos.videos.iter_mut() {
                if vid.url == self.toshow[self.i].url {
                    vid.flag = flag.clone();
                }
            }
            save_videos(&self.app_config, &self.videos);
        }
    }

    fn flag_unflag(&mut self) {
        if self.i < self.toshow.len() {
            let flag = match self.toshow[self.i].flag {
                Some(Flag::Read) => None, 
                None => Some(Flag::Read),
            };
            self.flag(&flag);
            self.clear_and_print_videos();
        }
    }

    fn help(&mut self) {
        clear();
        print_help();
        self.wait_key_press_and_clear_and_print_videos()
    }

    fn up(&mut self) {
        self.i = jump(self.i, if self.i > 0 { self.i - 1 } else { self.n - 1 });
    }

    fn down(&mut self) {
        self.i = jump(self.i, self.i + 1);
    }

    fn handle_resize(&mut self) {
        let lines = get_lines();
        if self.n != lines {
            self.n = lines;
            self.i = 0;
            self.clear_and_print_videos();
        }
    }

    async fn run(&mut self) {
        match load(false, &self.app_config, &self.videos).await {
            Some(videos) => self.videos = videos,
            None => debug("no video to load")
        };
        self.start = 0;
        self.i = 0;
        smcup();
        self.first_page();
        self.clear_and_print_videos();
        hide_cursor();
        loop {
            self.handle_resize();
            print_selector(self.i);
            let input = input();
            let result;
            {
                match input.enable_mouse_mode() {
                    Ok(_) => {},
                    Err(_) => {}
                }
                let _screen = RawScreen::into_raw_mode();
                let mut stdin = input.read_sync();
                result = stdin.next();
                match input.disable_mouse_mode() {
                    Ok(_) => {},
                    Err(_) => {}
                }
            }
            match result {
                None => (),
                Some(key_event) => {
                    match key_event {
                        InputEvent::Keyboard(event) => {
                            match event {
                                Ctrl('c') | Char('q') => {
                                    quit();
                                    break;
                                },
                                Char('c') => download_subscriptions(),
                                Char('j') | Char('l') | Down => self.down(),
                                Char('k') | Up => self.up(),
                                Char('g') | Char('H') => self.i = jump(self.i, 0),
                                Char('M') => self.i = jump(self.i, self.n / 2),
                                Char('G') | Char('L') => self.i = jump(self.i, self.n - 1),
                                Char('r') | Char('$') | Left => self.soft_reload(),
                                Char('P') => self.previous_page(),
                                Char('N') => self.next_page(),
                                Char('R') => self.hard_reload().await,
                                Char('h') | Char('?') => self.help(),
                                Char('i') | Right => self.info(),
                                Char('t') => self.flag_unflag(),
                                Char('T') => {match self.display_current_thumbnail().await {
                                    Ok(_) => {},
                                    Err(e) => debug(&format!("error: {:?}", e))
                                }},
                                Char('p') | Char('\n') => self.play_current(),
                                Char('o') => self.open_current(),
                                Char('/') => self.search(),
                                Char('n') => self.search_next(),
                                Char(':') => self.command(),
                                Char('y') => self.yank_video_uri(),
                                Char('f') | Char('|') => self.filter(),
                                _ => debug(&"key not supported (press h for help)".to_string()),
                            }
                        },
                        InputEvent::Mouse(event) => {
                            match event {
                                MouseEvent::Press(MouseButton::Left, _x, y) => {
                                    let new_i = usize::from(y) - 1;
                                    if self.i == new_i {
                                        self.play_current();
                                    }
                                    else {
                                        self.i = jump(self.i, new_i);
                                    }
                                },
                                MouseEvent::Press(MouseButton::WheelUp, _x, _y) => {
                                    self.up()
                                },
                                MouseEvent::Press(MouseButton::WheelDown, _x, _y) => {
                                    self.down()
                                },
                                _ => (),
                            }
                        },
                        _ => ()
                    }
                }
            }
            self.i %= self.n;
        };
    }
}

#[tokio::main]
async fn main() {
    let _ = ctrlc::set_handler(move || {
        quit();
        std::process::exit(0);
    });
    match Regex::new("") {
        Ok(empty_regex) => {
            let empty_regex_2 = empty_regex.clone();
            let mut yts = YoutubeSubscribtions{
                n: 0,
                start: 0,
                search: empty_regex,
                filter: empty_regex_2,
                i: 0,
                toshow: vec![],
                videos: Items{channel_etags: HashMap::new(), videos: vec![]},
                app_config: load_config_fallback(),
            };
        yts.run().await;
        },
        Err(_) => {
            println!("failed creating regex")
        }
    }
}
