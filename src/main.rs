extern crate base64;
extern crate blockish;
extern crate blockish_player;
extern crate chrono;
extern crate copypasta;
extern crate crossterm;
extern crate crossterm_input;
extern crate ctrlc;
extern crate dirs;
extern crate html2text;
extern crate percent_encoding;
extern crate reqwest;
extern crate roxmltree;
extern crate serde;

use blockish::render_image_fitting_terminal;
use chrono::DateTime;
use copypasta::{ClipboardContext, ClipboardProvider};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm_input::KeyEvent::{self, Char, Ctrl, Down, Left, Right, Up};
use crossterm_input::{input, InputEvent, MouseButton, MouseEvent, RawScreen};
use futures::future::join_all;
use percent_encoding::percent_decode;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, ETAG, IF_NONE_MATCH};
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::ErrorKind::NotFound;
use std::io::{BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime};
use tokio::sync::mpsc;
use utf8::BufReadDecoder;

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
    let mut symbols: HashMap<String, String> = HashMap::new();
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
    player_additional_opts: Vec<String>,
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
    auto_thumbnail_path: Option<String>,
}

impl Default for AppConfig {
    fn default() -> AppConfig {
        AppConfig {
            kind_symbols: default_kind_symbols(),
            video_path: "/tmp".to_string(),
            cache_path: "__HOME/.cache/yts/yts.json".to_string(),
            youtubedl_format: "[height <=? 360][ext = mp4]".to_string(),
            player_additional_opts: vec![],
            video_extension: "mp4".to_string(),
            blockish_player: None,
            players: vec![
                vec![
                    "/usr/bin/omxplayer".to_string(),
                    "-o".to_string(),
                    "local".to_string(),
                ],
                vec![
                    "/Applications/VLC.app/Contents/MacOS/VLC".to_string(),
                    "--play-and-exit".to_string(),
                    "-f".to_string(),
                ],
                vec![
                    "/usr/bin/vlc".to_string(),
                    "--play-and-exit".to_string(),
                    "-f".to_string(),
                ],
                vec![
                    "/usr/bin/mpv".to_string(),
                    "-really-quiet".to_string(),
                    "-fs".to_string(),
                ],
                vec![
                    "/usr/bin/mplayer".to_string(),
                    "-really-quiet".to_string(),
                    "-fs".to_string(),
                ],
            ],
            channel_ids: vec![],
            channel_urls: vec![],
            mpv_mode: true,
            mpv_path: "/usr/bin/mpv".to_string(),
            fs: true,
            open_magnet: None,
            sort: "desc".to_string(),
            auto_thumbnail_path: None,
        }
    }
}

fn load_config() -> Result<AppConfig, std::io::Error> {
    match dirs::home_dir() {
        Some(home) => match home.to_str() {
            Some(h) => {
                let path = format!("{}/.config/youtube-subscriptions/config.json", h);

                let mut _res = match fs::read_to_string(path) {
                    Ok(s) => serde_json::from_str::<AppConfig>(s.as_str())?,
                    _ => {
                        let config_path = format!("{}/.config/youtube-subscriptions/", h);
                        let config_file_path =
                            format!("{}/.config/youtube-subscriptions/config.json", h);
                        fs::create_dir_all(&config_path)?;
                        let mut file = File::create(config_file_path)?;
                        let default_config = AppConfig {
                            ..Default::default()
                        };
                        let config_as_string = serde_json::to_string(&default_config)?;
                        file.write_all(config_as_string.as_bytes())?;

                        AppConfig {
                            ..Default::default()
                        }
                    }
                };

                _res.video_path = _res.video_path.replace("__HOME", &h);
                fs::create_dir_all(&_res.video_path)?;

                let cache_path = format!("{}/.cache/yts/", h);
                fs::create_dir_all(&cache_path)?;
                Ok(_res)
            }
            None => Ok(AppConfig {
                ..Default::default()
            }),
        },
        None => Ok(AppConfig {
            ..Default::default()
        }),
    }
}

fn subscriptions_url() -> &'static str {
    "https://www.youtube.com/subscription_manager?action_takeout=1"
}

fn subscription_manager_relative_path() -> &'static str {
    ".config/youtube-subscriptions/subscription_manager"
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
    match app_config.kind_symbols.get(&format!("{:?}", kind)) {
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
        $node
            .descendants()
            .find(|n| n.tag_name().name() == $name)
            .unwrap_or($node)
    };
}

fn entry_to_item_rss(title: &String, channel_url: &str, entry: roxmltree::Node) -> Item {
    let mut kind = ItemKind::Other;
    let url = get_decendant_node!(entry, "link")
        .attribute("href")
        .unwrap_or("");
    let video_title = get_decendant_node!(entry, "title").text().unwrap_or("");
    let video_published = get_decendant_node!(entry, "published").text().unwrap_or(
        get_decendant_node!(entry, "updated")
            .text()
            .unwrap_or(get_decendant_node!(entry, "issued").text().unwrap_or("")),
    );
    let thumbnail = get_decendant_node!(entry, "thumbnail")
        .attribute("url")
        .unwrap_or("");
    if thumbnail != "".to_string() {
        kind = ItemKind::Video
    }
    let group = get_decendant_node!(entry, "group");
    let description = match get_decendant_node!(group, "description").text() {
        Some(stuff) => stuff,
        None => "",
    };
    let content = get_decendant_node!(group, "content")
        .text()
        .map(|x| x.to_string());
    Item {
        kind,
        content,
        channel: title.to_string(),
        title: video_title.to_string(),
        url: url.to_string(),
        published: video_published.to_string(),
        description: description.to_string(),
        thumbnail: thumbnail.to_string(),
        flag: default_flag(),
        channel_url: channel_url.to_string(),
    }
}

fn entry_to_item_atom(title: &str, channel_url: &str, entry: roxmltree::Node) -> Item {
    let mut kind = ItemKind::Other;
    let url = get_decendant_node!(entry, "enclosure")
        .attribute("url")
        .map(|x| {
            kind = if x.starts_with("magnet:") {
                ItemKind::Magnet
            } else {
                ItemKind::Audio
            };
            x
        })
        .unwrap_or(get_decendant_node!(entry, "link").text().unwrap_or(""));
    let video_title = get_decendant_node!(entry, "title").text().unwrap_or("");
    let video_published = get_decendant_node!(entry, "pubDate").text().unwrap_or("");
    let thumbnail = get_decendant_node!(entry, "thumbnail")
        .attribute("url")
        .unwrap_or("");
    if thumbnail != "".to_string() {
        kind = ItemKind::Video;
    }
    let description = get_decendant_node!(entry, "description")
        .text()
        .unwrap_or("");
    let date = match DateTime::parse_from_rfc2822(video_published) {
        Ok(x) => x.to_rfc3339(),
        Err(_) => chrono::offset::Local::now().to_rfc3339(),
    };
    let content = get_decendant_node!(entry, "encoded")
        .text()
        .map(|x| x.to_string());
    Item {
        kind,
        content,
        channel: title.to_string(),
        title: video_title.to_string(),
        url: url.to_string(),
        published: date,
        description: description.to_string(),
        thumbnail: thumbnail.to_string(),
        flag: default_flag(),
        channel_url: channel_url.to_string(),
    }
}

fn get_original_channel_videos(
    channel_url: &String,
    channel_etag: &Option<&String>,
    original_videos: &Items,
) -> Option<ChanelItems> {
    let mut channel_videos: Vec<Item> = vec![];
    for video in original_videos.videos.iter() {
        if &video.channel_url == channel_url {
            channel_videos.push(video.clone())
        }
    }
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
        Some(etag) => match HeaderValue::from_str(etag.as_str()) {
            Ok(s) => {
                headers.insert(IF_NONE_MATCH, s);
            }
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
        Ok(re) => match re.captures(channel_url) {
            Some(caps) => ChannelURLWithBasicAuth {
                channel_url: (format!("{}{}", caps[1].to_string(), caps[4].to_string()))
                    .to_string(),
                password: Some(
                    percent_decode(caps[3].as_bytes())
                        .decode_utf8_lossy()
                        .to_string(),
                ),
                user: Some(
                    percent_decode(caps[2].as_bytes())
                        .decode_utf8_lossy()
                        .to_string(),
                ),
            },
            None => ChannelURLWithBasicAuth {
                channel_url: channel_url.to_string(),
                password: None,
                user: None,
            },
        },
        Err(_) => ChannelURLWithBasicAuth {
            channel_url: channel_url.to_string(),
            password: None,
            user: None,
        },
    }
}

fn build_request(
    channel_url: &String,
    client: &reqwest::Client,
    channel_etag: Option<&String>,
) -> reqwest::RequestBuilder {
    let channel_url_with_basic_auth = parse_basic_auth(&channel_url);
    match channel_url_with_basic_auth.user {
        Some(user) => client
            .get(channel_url_with_basic_auth.channel_url.as_str())
            .headers(get_headers(channel_etag))
            .basic_auth(user, channel_url_with_basic_auth.password),
        None => client
            .get(channel_url_with_basic_auth.channel_url.as_str())
            .headers(get_headers(channel_etag)),
    }
}

fn to_show_videos(
    app_config: &AppConfig,
    videos: &mut Vec<Item>,
    start: usize,
    end: usize,
    filter: &Regex,
) -> Vec<Item> {
    videos.sort_by(|a, b| b.published.cmp(&a.published));
    let filtered_videos = videos
        .iter()
        .filter(|video| {
            filter.is_match(&format!("{:?}{}{}", video.kind, video.channel, video.title))
        })
        .cloned()
        .collect::<Vec<Item>>();
    let new_end = std::cmp::min(end, filtered_videos.len());
    let mut result = filtered_videos[start..new_end].to_vec();
    if app_config.sort == "desc" {
        result.reverse()
    }
    result
}

fn replace_home(path: &String) -> String {
    let home = dirs::home_dir().expect("home dir");
    let path = path.as_str();
    path.replace("__HOME", home.to_str().expect("home as str"))
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
        Ok(_) => {}
        Err(_) => {}
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
    move_cursor(0, 0);
}

fn show_cursor() {
    print!("\x1b[?25h");
    flush_stdout();
}

fn move_cursor(i: usize, j: usize) {
    print!("\x1b[{};{}f", i + 1, j + 1);
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

fn print_selector(i: usize, col_width: usize) {
    move_cursor(i, 0);
    print!("\x1b[1m|\x1b[0m\r");
    move_cursor(i, col_width);
    print!("\x1b[1m|\x1b[0m\r");
    flush_stdout();
}

fn clear_selector(i: usize, col_width: usize) {
    move_cursor(i, 0);
    print!(" ");
    move_cursor(i, col_width);
    print!(" ");
    flush_stdout();
}

fn jump(i: usize, new_i: usize, col_width: usize) -> usize {
    clear_selector(i, col_width);
    new_i
}

fn pause() {
    let input = input();
    let _screen = RawScreen::into_raw_mode();
    let _c = input.read_char();
}

struct YoutubeSubscribtions {
    modified: SystemTime,
    background_mode: bool,
    col_width: usize,
    n: usize,
    start: usize,
    search: Regex,
    filter: Regex,
    i: usize,
    toshow: Vec<Item>,
    videos: Items,
    app_config: AppConfig,
    filter_chars: Vec<char>,
}

fn print_press_any_key_and_pause() {
    println!("press any key to continue...");
    pause();
}

fn print_help() {
    println!(
        "\x1b[34;1m{}\x1b[0m {}",
        "youtube-subscriptions",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "\x1b[36m{}\x1b[0m",
        "a tool to view your video subscriptions in a terminal"
    );
    println!(
        "
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
  a          plays selected item audio only
  o          open selected video in browser
  t          tag untag a video as read
  T          display thumbnail
  y          copy video url in system clipboard
  c          download subscriptions default browser
  "
    )
}

fn split_cols(string: &str, cols: usize) -> Vec<String> {
    let mut chars = string.chars();
    (0..)
        .map(|_| chars.by_ref().take(cols).collect::<String>())
        .take_while(|s| !s.is_empty())
        .collect::<Vec<_>>()
}

fn info_lines(v: &Item) -> Vec<String> {
    let cols = get_cols();
    let mut lines: Vec<String> = vec![];
    lines.push(format!("\x1b[34;1m{}\x1b[0m", v.title));
    lines.push("".to_string());
    lines.push(format!("from \x1b[36m{}\x1b[0m", v.channel));
    lines.push("".to_string());
    v.description.split("\n").for_each(|x| {
        split_cols(&x, cols)
            .iter()
            .for_each(|y| lines.push(y.to_string()))
    });
    match &v.content {
        Some(x) => {
            lines.push("".to_string());
            html2text::from_read(x.as_bytes(), cols)
                .split("\n")
                .for_each(|x| lines.push(x.to_string()));
        }
        None => {}
    }
    lines
}

fn print_tildeline() {
    println!("\x1b[34;1m~\x1b[0m");
}

fn print_lines(lines: &Vec<String>, start: usize, rows: usize) {
    let stop = min(lines.len(), start + rows);
    for (_, line) in lines[start..stop].iter().enumerate() {
        println!("{}", line);
    }
    if stop >= start {
        for _ in (stop - start)..rows {
            print_tildeline();
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
            Some(key_event) => match key_event {
                InputEvent::Keyboard(event) => match event {
                    Char('q') | Left => {
                        break;
                    }
                    Char('g') => {
                        i = 0;
                    }
                    Char('G') => {
                        i = lines.len() - 1;
                    }
                    Char('k') | Up => {
                        if i > 0 {
                            i = i - 1
                        };
                    }
                    Char('j') | Down => {
                        if i < lines.len() {
                            i = i + 1
                        };
                    }
                    Ctrl('u') => {
                        if i > delta {
                            i = i - delta
                        } else {
                            i = 0
                        };
                    }
                    Ctrl('d') => {
                        if i + delta < lines.len() {
                            i = i + delta
                        } else {
                            i = lines.len()
                        };
                    }
                    _ => {}
                },
                _ => (),
            },
        }
    }
}

fn print_info(v: &Item) {
    less(info_lines(v));
}

fn quit() {
    show_cursor();
    rmcup();
}

fn chinese_chars(string: &str) -> usize {
    string.chars().fold(0, |acc, ch| {
        acc + (
            // Check if the character is a Chinese character
            if ch >= '\u{4E00}' && ch <= '\u{9FFF}' {
                1
            } else {
                0
            }
        )
    })
}

fn count_chars(string: &str) -> usize {
    string.chars().fold(0, |acc, ch| {
        acc + (
            // Check if the character is a Chinese character
            if ch >= '\u{4E00}' && ch <= '\u{9FFF}' {
                2
            } else {
                1
            }
        )
    })
}

impl YoutubeSubscribtions {
    fn print_videos(&mut self) {
        let cols = get_cols();
        let rows = get_lines();
        let channel_max_size = cols / 3;
        let max = self.toshow.iter().fold(0, |acc, x| {
            std::cmp::max(
                std::cmp::min(count_chars(&x.channel), channel_max_size),
                acc,
            )
        });
        self.col_width = max + 11;
        for video in &self.toshow {
            let published = video.published.split('T').collect::<Vec<&str>>();
            let whitespaces =
                " ".repeat(max - std::cmp::min(count_chars(&video.channel), channel_max_size));
            let channel_short = video
                .channel
                .chars()
                .take(channel_max_size)
                .collect::<String>();
            let published_short = if published.len() > 0 && published[0].len() >= 10 {
                published[0][5..10].to_string()
            } else {
                "?? ??".to_string()
            };
            let s = format!(
                " {} {} \x1b[36m{}\x1b[0m \x1b[34m{}\x1b[0m{}  {}",
                flag_to_string(&video.flag),
                kind_symbol(&self.app_config, &video.kind),
                published_short,
                channel_short,
                whitespaces,
                video.title
            );
            println!(
                "{}",
                s.chars()
                    .take(min(count_chars(&s), cols - chinese_chars(&s) - 4 + 9 + 9))
                    .collect::<String>()
            );
        }
        if self.toshow.len() < rows {
            for _ in 0..(rows - self.toshow.len()) {
                print_tildeline();
            }
        }
    }
    fn clear_and_print_videos(&mut self) {
        clear();
        self.print_videos()
    }

    fn download_subscriptions(&self) {
        let _res = webbrowser::open(&subscriptions_url());
        self.debug(&format!(
            "please save file to ~/{}",
            subscription_manager_relative_path()
        ));
    }

    fn get_subscriptions_xml(&self) -> Result<String, std::io::Error> {
        match dirs::home_dir() {
            Some(home) => match home.to_str() {
                Some(s) => {
                    let path = format!("{}/{}", s, subscription_manager_relative_path());
                    if fs::metadata(&path).is_ok() {
                        fs::read_to_string(path)
                    } else {
                        Ok("<opml></opml>".to_string())
                    }
                }
                None => {
                    self.debug("failed conversting home to str");
                    Ok("<opml></opml>".to_string())
                }
            },
            None => {
                self.debug("failed finding home dir");
                Ok("<opml></opml>".to_string())
            }
        }
    }

    fn get_title(&self, document: &roxmltree::Document) -> String {
        match document
            .descendants()
            .find(|n| n.tag_name().name() == "title")
        {
            Some(node) => node.text().unwrap_or(""),
            None => {
                self.debug("did not find title node");
                ""
            }
        }
        .to_string()
    }

    fn get_rss_videos(&self, document: roxmltree::Document, channel_url: &str) -> Vec<Item> {
        let title = self.get_title(&document);
        document
            .descendants()
            .filter(|n| n.tag_name().name() == "entry")
            .map(|entry| entry_to_item_rss(&title, channel_url, entry))
            .collect::<Vec<Item>>()
    }

    fn get_channel_videos_from_contents(&self, contents: &str, channel_url: &String) -> Vec<Item> {
        match roxmltree::Document::parse(contents) {
            Ok(document) => match document
                .descendants()
                .find(|n| n.tag_name().name() == "channel")
            {
                Some(channel) => self.get_atom_videos(channel, &channel_url),
                None => self.get_rss_videos(document, &channel_url),
            },
            Err(e) => {
                self.debug(&format!("failed parsing xml {}", e));
                vec![]
            }
        }
    }

    fn get_atom_videos(&self, channel: roxmltree::Node, channel_url: &String) -> Vec<Item> {
        let title = get_decendant_node!(channel, "title").text().unwrap_or("");
        channel
            .descendants()
            .filter(|n| n.tag_name().name() == "item")
            .map(|entry| entry_to_item_atom(&title, channel_url, entry))
            .collect::<Vec<Item>>()
    }

    async fn get_videos(
        &self,
        xml: String,
        additional_channel_ids: &[String],
        additional_channel_urls: &[String],
        original_videos: &Items,
    ) -> Vec<Option<ChanelItems>> {
        match roxmltree::Document::parse(xml.as_str()) {
            Ok(document) => {
                let mut urls_from_xml: Vec<String> = document
                    .descendants()
                    .filter(|n| n.tag_name().name() == "outline")
                    .map(|entry| entry.attribute("xmlUrl"))
                    .filter_map(|x| x)
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>();
                let urls_from_additional = additional_channel_ids.iter().map(|id| {
                    "https://www.youtube.com/feeds/videos.xml?channel_id=".to_string() + id
                });
                let urls_from_additional_2 =
                    additional_channel_urls.iter().map(|url| url.to_string());
                urls_from_xml.extend(urls_from_additional);
                urls_from_xml.extend(urls_from_additional_2);
                match reqwest::Client::builder().use_rustls_tls().build() {
                    Ok(client) => {
                        let futs: Vec<_> = urls_from_xml
                            .iter()
                            .map(|url| {
                                let etag = match original_videos.channel_etags.get(&url.to_string())
                                {
                                    Some(Some(string)) => Some(string),
                                    _ => None,
                                };
                                self.get_channel_videos(
                                    &client,
                                    url.to_string(),
                                    etag,
                                    &original_videos,
                                )
                            })
                            .collect();
                        join_all(futs).await
                    }
                    Err(e) => {
                        self.debug(&format!("failed instantiating client {}", e));
                        vec![None]
                    }
                }
            }
            Err(e) => {
                self.debug(&format!("failed parsing xml {}", e));
                vec![None]
            }
        }
    }

    async fn get_channel_videos(
        &self,
        client: &reqwest::Client,
        channel_url: String,
        channel_etag: Option<&String>,
        original_videos: &Items,
    ) -> Option<ChanelItems> {
        let max_tries = 5;
        for i in 0..max_tries {
            let request = build_request(&channel_url, &client, channel_etag);
            let wrapped_response: Result<reqwest::Response, reqwest::Error> = request.send().await;
            match wrapped_response {
                Ok(response) => {
                    let status = response.status();
                    if status.as_u16() == 304 {
                        return get_original_channel_videos(
                            &channel_url,
                            &channel_etag,
                            &original_videos,
                        );
                    } else if status.is_success() {
                        self.debug(&format!("💚 success loading {}", &channel_url));
                        let headers = response.headers();
                        let etag_opt_opt = headers
                            .get(ETAG)
                            .map(|x| x.to_str().ok().map(|y| y.to_string()));
                        match response.text().await {
                            Ok(text) => {
                                return Some(ChanelItems {
                                    channel_url: channel_url.to_string(),
                                    etag: match etag_opt_opt {
                                        Some(Some(x)) => Some(x),
                                        _ => None,
                                    },
                                    videos: self
                                        .get_channel_videos_from_contents(&text, &channel_url),
                                })
                            }
                            Err(_) => {}
                        }
                    }
                }
                Err(e) if i == (max_tries - 1) => {
                    self.debug(&format!("🔴 failed loading {}: {}", &channel_url, e))
                }
                Err(e) => self.debug(&format!(
                    "🟠 retrying after fail #{} for {}: {:?}",
                    i,
                    &channel_url,
                    e.status()
                )),
            }
            let dur = std::time::Duration::from_millis(i * 100);
            std::thread::sleep(dur);
        }
        None
    }

    async fn load(
        &self,
        reload: bool,
        app_config: &AppConfig,
        original_videos: &Items,
    ) -> Option<Items> {
        match self.get_subscriptions_xml() {
            Ok(xml) => {
                let path = replace_home(&app_config.cache_path);
                if reload || fs::metadata(&path).is_err() {
                    let mut one_query_failed = false;
                    let empty_vec = vec![];
                    let mut etags: ChannelEtags = HashMap::new();
                    let vids = self
                        .get_videos(
                            xml,
                            &app_config.channel_ids,
                            &app_config.channel_urls,
                            &original_videos,
                        )
                        .await
                        .iter()
                        .map(|x| match x.as_ref() {
                            Some(res) => {
                                etags.insert(res.channel_url.clone(), res.etag.clone());
                                &res.videos
                            }
                            None => {
                                one_query_failed = true;
                                &empty_vec
                            }
                        })
                        .flat_map(|x| x)
                        .cloned()
                        .collect::<Vec<Item>>();
                    if one_query_failed {
                        return None;
                    }
                    let mut videos = Items {
                        channel_etags: etags,
                        videos: vids,
                    };

                    for vid in videos.videos.iter_mut() {
                        for original_vid in original_videos.videos.iter() {
                            if vid.url == original_vid.url {
                                vid.flag = original_vid.flag.clone();
                            }
                        }
                    }
                    self.save_videos(app_config, &videos);
                    Some(videos)
                } else {
                    match fs::read_to_string(&path) {
                        Ok(s) => match serde_json::from_str(s.as_str()) {
                            Ok(res) => Some(res),
                            Err(e) => {
                                self.debug(&format!("failed reading {} {}", path, e));
                                None
                            }
                        },
                        Err(_) => None,
                    }
                }
            }
            Err(_) => None,
        }
    }

    fn save_videos(&self, app_config: &AppConfig, videos: &Items) {
        let proper_path = replace_home(&app_config.cache_path);
        match serde_json::to_string(&videos) {
            Ok(serialized) => match fs::write(&proper_path, serialized) {
                Ok(_) => {}
                Err(e) => {
                    self.debug(&format!("failed writing {} {}", &proper_path, e));
                }
            },
            Err(e) => {
                self.debug(&format!("failed serializing videos {}", e));
            }
        }
    }

    fn play_video_usual(&self, path: &str, app_config: &AppConfig) {
        for player in &app_config.players {
            if fs::metadata(&player[0]).is_ok() {
                let mut child1 = Command::new(&player[0]);
                for arg in player.iter().skip(1) {
                    child1.arg(&arg);
                }
                self.read_command_output(child1.arg(path), &player[0]);
                return;
            }
        }
    }

    fn play_video(&self, path: &str, app_config: &AppConfig) {
        match &app_config.blockish_player {
            None => self.play_video_usual(&path, &app_config),
            Some(player) => {
                match blockish_player::video_command(player, &path.to_string()) {
                    Ok(mut command) => {
                        self.read_command_output(&mut command, &"blockish_player".to_string());
                    }
                    Err(e) => {
                        self.debug(&format!("error: {:?}", e));
                        self.play_video_usual(&path, &app_config);
                    }
                };
            }
        }
    }

    fn download_video(&self, path: &str, id: &str, app_config: &AppConfig) {
        if fs::metadata(&path).is_err() {
            self.read_command_output(
                Command::new("youtube-dl")
                    .arg("-f")
                    .arg(&app_config.youtubedl_format)
                    .arg("-o")
                    .arg(&path)
                    .arg("--")
                    .arg(&id),
                &"youtube-dl".to_string(),
            )
        }
    }

    fn open_magnet(&self, url: &str, app_config: &AppConfig) {
        match &app_config.open_magnet {
            Some(open_magnet) => {
                self.read_command_output(Command::new(open_magnet).arg(&url), &open_magnet)
            }
            None => {}
        }
    }

    fn read_command_output(&self, command: &mut Command, binary: &str) {
        match command.stdout(Stdio::piped()).spawn() {
            Ok(mut child) => {
                let stdout_option = child.stdout.take();
                match stdout_option {
                    Some(stdout) => {
                        let mut decoder = BufReadDecoder::new(BufReader::new(stdout));
                        loop {
                            let read_result = decoder.next_strict();
                            match read_result {
                                Some(Ok(s)) => {
                                    print!("{}", s);
                                }
                                Some(Err(_)) => {}
                                None => {
                                    break;
                                }
                            }
                        }
                    }
                    None => self.debug("no stdout"),
                }
                let stderr_option = child.stderr.take();
                match stderr_option {
                    Some(stderr) => {
                        for byte_result in stderr.bytes() {
                            match byte_result {
                                Ok(byte) => {
                                    print!("{}", byte as char);
                                    flush_stdout();
                                }
                                Err(_) => {}
                            }
                        }
                    }
                    None => self.debug("no stderr"),
                }
                match child.wait() {
                    Ok(status) => {
                        if !status.success() {
                            println!(
                                "error while running {:?}, return status: {:?}",
                                command,
                                status.code()
                            );
                            print_press_any_key_and_pause()
                        }
                    }
                    Err(e) => {
                        println!("error while running {:?}, error: {:?}", command, e);
                        print_press_any_key_and_pause()
                    }
                }
            }
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

    fn debug(&self, s: &str) {
        if self.background_mode {
            let cols = get_cols();
            println!("{}", s.chars().take(cols - 2).collect::<String>());
        } else {
            move_to_bottom();
            clear_to_end_of_line();
            move_to_bottom();
            print!("{}", s);
            flush_stdout();
        }
    }

    fn move_page(&mut self, direction: i8) {
        self.n = get_lines();
        if direction == 1 {
            if self.start + 2 * self.n < self.videos.videos.len() {
                self.start += self.n;
            }
        } else if direction == 0 {
            self.start = 0;
        } else if direction == -1 {
            if self.n > self.start {
                self.start = 0;
            } else {
                self.start -= self.n;
            }
        }
        self.toshow = to_show_videos(
            &self.app_config,
            &mut self.videos.videos,
            self.start,
            self.start + self.n,
            &self.filter,
        );
        self.i = 0;
        self.clear_and_print_videos()
    }

    fn next_page(&mut self) {
        self.move_page(-1);
    }

    fn previous_page(&mut self) {
        self.move_page(1);
    }

    fn cache_modified(&self) -> bool {
        let path = replace_home(&self.app_config.cache_path);
        match fs::metadata(&path) {
            Ok(metadata) => {
                let modified = metadata.modified().unwrap();
                if self.modified < modified {
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    async fn soft_reload(&mut self) {
        if self.cache_modified() {
            self.load_videos_from_cache().await;
        }
        self.move_page(0);
    }

    async fn hard_reload(&mut self) {
        let now = Instant::now();
        self.debug(&"updating video list...".to_string());
        match self.load(true, &self.app_config, &self.videos).await {
            Some(videos) => self.videos = videos,
            None => self.debug("could not load videos"),
        }
        self.debug(&"".to_string());
        self.debug(&format!("reload took {} ms", now.elapsed().as_millis()).to_string());
    }

    fn first_page(&mut self) {
        self.n = get_lines();
        self.toshow = to_show_videos(
            &self.app_config,
            &mut self.videos.videos,
            self.start,
            self.n,
            &self.filter,
        );
    }

    fn play_current(&mut self, no_video: bool) {
        if self.i < self.toshow.len() {
            self.play(&self.toshow[self.i], &self.app_config, no_video);
            self.flag(&Some(Flag::Read));
            self.clear_and_print_videos();
        }
    }

    async fn write_thumbnail(&mut self, i: usize) -> Result<String, CustomError> {
        let url = &self.toshow[i].thumbnail;
        let path = format!(
            "{}/{}.{}",
            self.app_config.video_path,
            base64::encode(&url),
            ".jpg"
        );
        if fs::metadata(&path).is_ok() {
            return Ok(path);
        }
        let resp = reqwest::get(url).await?;
        let mut out = File::create(&path)?;
        let bytes = resp.bytes().await?;
        out.write_all(&bytes[..])?;
        Ok(path)
    }

    async fn display_current_thumbnail(&mut self) -> Result<(), CustomError> {
        if self.i < self.toshow.len() {
            let path = self.write_thumbnail(self.i).await?;
            clear();
            render_image_fitting_terminal(&path);
            self.wait_key_press_and_clear_and_print_videos();
        }
        Ok(())
    }

    async fn current_thumbnail_to_thumbnail_jpg(&mut self) -> Result<(), CustomError> {
        match self.app_config.auto_thumbnail_path.clone() {
            Some(thumbnail_path) => {
                if self.i < self.toshow.len() {
                    let i = self.i;
                    let path = self.write_thumbnail(i).await?;
                    // copy path to ytsthumbnail.jpg
                    let channel = &self.toshow[i].channel;
                    let title = &self.toshow[i].title;
                    // write channel and title to a $thumbnail_path.jpg.txt
                    let mut file = File::create(format!("{}.txt", thumbnail_path))?;
                    file.write_all(channel.as_bytes())?;
                    file.write_all(b"\n")?;
                    file.write_all(title.as_bytes())?;
                    file.flush()?;
                    fs::copy(&path, &thumbnail_path)?;
                }
            }
            None => {}
        }
        Ok(())
    }

    fn open_current(&mut self) {
        if self.i < self.toshow.len() {
            let url = &self.toshow[self.i].url;
            self.debug(&format!("opening {}", &url));
            let _res = webbrowser::open(&url);
            self.flag(&Some(Flag::Read));
            self.clear_and_print_videos();
        }
    }

    fn find_next(&mut self) -> usize {
        for (i, video) in self.toshow.iter().enumerate() {
            if i > self.i
                && (self.search.is_match(&video.title) || self.search.is_match(&video.channel))
            {
                return i;
            }
        }
        self.i
    }

    fn realtime_input_with_prefix(&mut self, start_symbol: &str) -> Option<String> {
        move_to_bottom();
        clear_to_end_of_line();
        print!("{}", start_symbol);
        for c in self.filter_chars.iter() {
            print!("{}", c);
        }
        flush_stdout();
        let _ = enable_raw_mode();
        let input = input();
        let mut reader = input.read_sync();
        match reader.next() {
            Some(InputEvent::Keyboard(KeyEvent::Backspace)) => {
                self.filter_chars.pop();
            }
            Some(InputEvent::Keyboard(KeyEvent::Enter)) => {
                let _ = disable_raw_mode();
                return None;
            }
            Some(InputEvent::Keyboard(KeyEvent::Char(c))) => {
                print!("{}", c);
                self.filter_chars.push(c)
            }
            _ => {}
        }
        let _ = disable_raw_mode();
        Some(self.filter_chars.iter().collect::<String>())
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
        clear_selector(self.i, self.col_width);
        self.i = self.find_next();
    }

    fn search(&mut self) {
        let s = self.input_with_prefix("/");
        match Regex::new(&format!(".*(?i){}.*", s)) {
            Ok(regex) => {
                self.search = regex;
                self.i = self.find_next();
            }
            Err(_) => self.debug("failing creating regex"),
        }
        self.clear_and_print_videos()
    }

    fn set_filter(&mut self, s: &String) {
        let wildcard_s = s.replace("", ".*");
        match Regex::new(&format!(".*(?i){}.*", wildcard_s)) {
            Ok(regex) => {
                self.filter = regex;
                self.move_page(0);
            }
            Err(_) => self.debug("failing creating regex"),
        }
        self.clear_and_print_videos();
    }

    fn filter(&mut self) {
        self.filter_chars = vec![];
        self.set_filter(&"".into());
        loop {
            match self.realtime_input_with_prefix("|") {
                Some(s) => self.set_filter(&s),
                None => {
                    break;
                }
            }
        }
    }

    fn play(&self, v: &Item, app_config: &AppConfig, no_video: bool) {
        self.play_url(&v.url, &v.kind, app_config, no_video);
    }

    fn play_url(&self, url: &String, kind: &ItemKind, app_config: &AppConfig, no_video: bool) {
        if app_config.mpv_mode && fs::metadata(&app_config.mpv_path).is_ok() {
            let message = format!("playing {} with mpv...", url);
            self.debug(&message);
            clear();
            match Command::new(&app_config.mpv_path)
                .args(&app_config.player_additional_opts)
                .arg(if no_video { "--no-video" } else { "" })
                .arg(if app_config.fs { "-fs" } else { "" })
                .arg("--ytdl-format=".to_owned() + &app_config.youtubedl_format)
                .arg(&url)
                .spawn()
            {
                Ok(mut child) => match child.wait() {
                    Ok(_) => {}
                    Err(e) => {
                        self.debug(&format!("{}", e));
                    }
                },
                _ => {}
            };
        } else {
            clear();
            match kind {
                ItemKind::Audio => {
                    self.play_video(&url, app_config);
                }
                ItemKind::Magnet => {
                    self.open_magnet(&url, app_config);
                }
                _ => {
                    let path = format!(
                        "{}/{}.{}",
                        app_config.video_path,
                        base64::encode(&url),
                        app_config.video_extension
                    );
                    self.download_video(&path, &url, app_config);
                    self.play_video(&path, app_config);
                }
            }
        }
    }

    fn command(&mut self) {
        let s = self.input_with_prefix(":");
        let s = s.split_whitespace().collect::<Vec<&str>>();
        hide_cursor();
        clear();
        if s.len() == 2 {
            if let "o" = s[0] {
                self.play_url(&s[1].to_string(), &ItemKind::Video, &self.app_config, false)
            }
        }
        self.clear_and_print_videos()
    }

    fn yank_video_uri(&mut self) {
        let url = &self.toshow[self.i].url;
        match ClipboardContext::new() {
            Ok(mut ctx) => match ctx.set_contents(url.to_string()) {
                Ok(_) => self.debug(&format!("yanked {}", url)),
                Err(e) => self.debug(&format!("failed yanking {}: {}", url, e)),
            },
            Err(e) => self.debug(&format!("error: {:?}", e)),
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
            self.save_videos(&self.app_config, &self.videos);
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
        self.i = jump(
            self.i,
            if self.i > 0 { self.i - 1 } else { self.n - 1 },
            self.col_width,
        );
    }

    fn down(&mut self) {
        self.i = jump(self.i, self.i + 1, self.col_width);
    }

    fn handle_resize(&mut self) {
        let lines = get_lines();
        if self.n != lines {
            self.n = lines;
            self.i = 0;
            self.clear_and_print_videos();
        }
    }

    async fn load_videos_from_cache(&mut self) {
        match self.load(false, &self.app_config, &self.videos).await {
            Some(videos) => {
                self.videos = videos;
                self.modified = SystemTime::now();
            }
            None => self.debug("no video to load"),
        };
    }
    async fn run(&mut self, sender: mpsc::Sender<()>, mut receiver: mpsc::Receiver<()>) {
        self.load_videos_from_cache().await;
        self.start = 0;
        self.i = 0;
        smcup();
        self.first_page();
        self.clear_and_print_videos();
        hide_cursor();
        let mut numbers: Vec<i64> = vec![];
        loop {
            if let Ok(_) = receiver.try_recv() {
                self.soft_reload().await;
                self.debug(&"reload done".to_string());
            }
            if self.videos.videos.len() == 0 {
                self.help();
            }
            self.handle_resize();
            print_selector(self.i, self.col_width);
            let input = input();
            let result;
            {
                match input.enable_mouse_mode() {
                    Ok(_) => {}
                    Err(_) => {}
                }
                let _screen = RawScreen::into_raw_mode();
                let mut stdin = input.read_sync();
                result = stdin.next();
                match input.disable_mouse_mode() {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            match result {
                None => (),
                Some(key_event) => match key_event {
                    InputEvent::Keyboard(event) => match event {
                        Char(x) if x.is_digit(10) => {
                            x.to_digit(10).map(|digit| numbers.push(digit as i64));
                        }
                        _ => {
                            let mut n: i64 = 0;
                            for i in 0..numbers.len() {
                                n = n * 10 + numbers[i];
                            }
                            let n = if n == 0 { 1 } else { n };
                            let mut quitting = false;
                            for _ in 0..n {
                                match event {
                                    Ctrl('c') | Char('q') => {
                                        quit();
                                        quitting = true;
                                    }
                                    Char('c') => self.download_subscriptions(),
                                    Char('j') | Char('l') | Down => {
                                        self.down();
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('k') | Up => {
                                        self.up();
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('g') | Char('H') => {
                                        self.i = jump(self.i, 0, self.col_width);
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('M') => {
                                        self.i = jump(self.i, self.n / 2, self.col_width);
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('G') | Char('L') => {
                                        self.i = jump(self.i, self.n - 1, self.col_width);
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('r') | Char('$') | Left => self.soft_reload().await,
                                    Char('P') => {
                                        self.previous_page();
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('N') => {
                                        self.next_page();
                                        let _ = self.current_thumbnail_to_thumbnail_jpg().await;
                                    }
                                    Char('R') => {
                                        tokio::spawn(hard_reload_bg(sender.clone()));
                                    }
                                    Char('h') | Char('?') => self.help(),
                                    Char('i') | Right => self.info(),
                                    Char('t') => self.flag_unflag(),
                                    Char('T') => match self.display_current_thumbnail().await {
                                        Ok(_) => {}
                                        Err(e) => self.debug(&format!("error: {:?}", e)),
                                    },
                                    Char('p') | KeyEvent::Enter => self.play_current(false),
                                    Char('a') => self.play_current(true),
                                    Char('o') => self.open_current(),
                                    Char('/') => self.search(),
                                    Char('n') => self.search_next(),
                                    Char(':') => self.command(),
                                    Char('y') => self.yank_video_uri(),
                                    Char('f') | Char('|') => self.filter(),
                                    _ => self
                                        .debug(&"key not supported (press h for help)".to_string()),
                                }
                                numbers = vec![];
                            }
                            if quitting {
                                break;
                            }
                        }
                    },
                    InputEvent::Mouse(event) => match event {
                        MouseEvent::Press(MouseButton::Left, _x, y) => {
                            let new_i = usize::from(y) - 1;
                            if self.i == new_i {
                                self.play_current(false);
                            } else {
                                self.i = jump(self.i, new_i, self.col_width);
                            }
                        }
                        MouseEvent::Press(MouseButton::WheelUp, _x, _y) => self.up(),
                        MouseEvent::Press(MouseButton::WheelDown, _x, _y) => self.down(),
                        _ => (),
                    },
                    _ => (),
                },
            }
            self.i %= self.n;
        }
    }
}

fn build_yts() -> YoutubeSubscribtions {
    YoutubeSubscribtions {
        modified: SystemTime::now(),
        background_mode: false,
        col_width: 0,
        n: 0,
        start: 0,
        search: Regex::new("").unwrap(),
        filter: Regex::new("").unwrap(),
        i: 0,
        toshow: vec![],
        videos: Items {
            channel_etags: HashMap::new(),
            videos: vec![],
        },
        app_config: load_config().expect("loaded config"),
        filter_chars: vec![],
    }
}

async fn hard_reload_bg(sender: mpsc::Sender<()>) {
    build_yts().hard_reload().await;
    let _ = sender.send(()).await;
}

#[tokio::main]
async fn main() {
    let _ = ctrlc::set_handler(move || {
        quit();
        std::process::exit(0);
    });
    let mut yts = build_yts();
    if yts.background_mode {
        println!("updating cache with new videos...");
        yts.hard_reload().await;
    } else {
        let (sender, receiver) = mpsc::channel::<()>(1);
        yts.run(sender, receiver).await;
    }
}
