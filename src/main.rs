extern crate dirs;
extern crate reqwest;
extern crate terminal_size;
extern crate crossterm_input;
extern crate crossterm;
extern crate serde;
extern crate clipboard;
extern crate roxmltree;
extern crate chrono;
extern crate ctrlc;
extern crate base64;
extern crate html2text;

use std::time::Instant;
use clipboard::{ClipboardProvider, ClipboardContext};
use serde::{Serialize, Deserialize};
use std::fs;
use std::io;
use std::path::Path;
use std::io::{Read, Write};
use std::io::Error;
use std::io::ErrorKind::NotFound;
use terminal_size::{Width, Height, terminal_size};
use std::cmp::min;
use std::process::{Command, Stdio};
use crossterm_input::{input, RawScreen, InputEvent, MouseEvent, MouseButton};
use crossterm_input::KeyEvent::{Char, Down, Up, Left, Right, Ctrl};
use futures::future::join_all;
use chrono::DateTime;
use regex::Regex;


use webbrowser;

fn default_mpv_mode() -> bool {
    true
}

fn default_content() -> Option<String> {
    None
}

fn default_channel_urls() -> Vec<String> {
    vec![]
}

fn default_mpv_path() -> String {
    "/usr/bin/mpv".to_string()
}

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    video_path: String,
    cache_path: String,
    youtubedl_format: String,
    video_extension: String,
    players: Vec<Vec<String>>,
    channel_ids: Vec<String>,
    #[serde(default = "default_channel_urls")]
    channel_urls: Vec<String>,
    #[serde(default = "default_mpv_mode")]
    mpv_mode: bool,
    #[serde(default = "default_mpv_path")]
    mpv_path: String,
}

impl Default for AppConfig {
    fn default() -> AppConfig {
        AppConfig {
            video_path: "/tmp".to_string(),
            cache_path: "__HOME/.cache/yts/yts.json".to_string(),
            youtubedl_format: "[height <=? 360][ext = mp4]".to_string(),
            video_extension: "mp4".to_string(),
            players: vec![
                vec!["/usr/bin/omxplayer".to_string(), "-o".to_string(), "local".to_string()],
                vec!["/Applications/VLC.app/Contents/MacOS/VLC".to_string(), "--play-and-exit".to_string(), "-f".to_string()],
                vec!["/usr/bin/vlc".to_string(), "--play-and-exit".to_string(), "-f".to_string()],
                vec!["/usr/bin/mpv".to_string(), "-really-quiet".to_string(), "-fs".to_string()],
                vec!["/usr/bin/mplayer".to_string(), "-really-quiet".to_string(), "-fs".to_string()],
            ],
            channel_ids: vec![],
            channel_urls: default_channel_urls(),
            mpv_mode: default_mpv_mode(),
            mpv_path: default_mpv_path(),
        }
    }
}

fn load_config() -> AppConfig {
    match dirs::home_dir() {
        Some(home) => {
            match home.to_str() {
                Some(h) => {
                    let path = format!("{}/.config/youtube-subscriptions/config.json",
                                       h);
                    match fs::read_to_string(path) {
                        Ok(s) => {
                            match serde_json::from_str::<AppConfig>(s.as_str()) {
                                Ok(mut _res) => {
                                    _res.video_path = _res.video_path.replace("__HOME", &h);
                                    match fs::create_dir_all(&_res.video_path) {
                                        Ok(_) => {
                                            _res.cache_path = _res.cache_path.replace("__HOME", &h);
                                            match Path::new(&_res.cache_path).parent() {
                                                Some(dirname) => match fs::create_dir_all(&dirname) {
                                                    Ok(_) => _res,
                                                    Err(e) => {
                                                        debug(&format!("error while creating cache directory for {}: {:?}", &_res.cache_path, e));
                                                        _res
                                                    }
                                                }
                                                None => {
                                                    debug(&format!("failed to find dirname of {}", &_res.cache_path));
                                                    _res
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            debug(&format!("error while creating video path {}: {:?}", &_res.video_path, e));
                                            _res
                                        }
                                    }
                                }
                                Err(e) => {
                                    debug(&format!("error parsing configuration: {:?}", e));
                                    AppConfig { ..Default::default() }
                                }
                            }
                        },
                        Err(_) =>
                            AppConfig { ..Default::default() }
                    }
                }
                None => AppConfig { ..Default::default() }
            }
        },
        None =>
            AppConfig { ..Default::default() }
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

fn get_subscriptions_xml() -> Result<String, Error> {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Video {
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

#[derive(Serialize, Deserialize, Debug)]
struct Videos {
    videos: Vec<Video>,
}

macro_rules! get_decendant_node {
    ( $node:expr, $name:expr  ) => {
        $node.descendants().find(|n| n.tag_name().name() == $name).unwrap_or($node)
    }
}

fn get_youtube_channel_videos(document: roxmltree::Document) -> Vec<Video> {
    let title = match document.descendants().find(|n| n.tag_name().name() == "title") {
        Some(node) => node.text().unwrap_or(""),
        None => {
            debug("did not find title node");
            ""
        }
    };
    document.descendants().filter(|n| n.tag_name().name() == "entry").map(|entry| {
        let url = get_decendant_node!(entry, "link").attribute("href").unwrap_or("");
        let video_title = get_decendant_node!(entry, "title").text().unwrap_or("");
        let video_published = get_decendant_node!(entry, "published").text().unwrap_or("");
        let thumbnail = get_decendant_node!(entry, "thumbnail").attribute("url").unwrap_or("");
        let group = get_decendant_node!(entry, "group");
        let description = match get_decendant_node!(group, "description").text() {
            Some(stuff) => stuff,
            None => "",
        };
        let content = get_decendant_node!(group, "content").text().map(|x| x.to_string());
        Video { 
            channel: title.to_string(),
            title: video_title.to_string(),
            url: url.to_string(),
            published: video_published.to_string(),
            description: description.to_string(),
            thumbnail: thumbnail.to_string(),
            flag: default_flag(),
            content: content,
        }
    }).collect::<Vec<Video>>()
}

fn get_peertube_channel_videos(channel: roxmltree::Node) -> Vec<Video> {
    let title = get_decendant_node!(channel, "title").text().unwrap_or("");
    channel.descendants().filter(|n| n.tag_name().name() == "item").map(|entry| {
        let url = get_decendant_node!(entry, "link").text().unwrap_or("");
        let video_title = get_decendant_node!(entry, "title").text().unwrap_or("");
        let video_published = get_decendant_node!(entry, "pubDate").text().unwrap_or("");
        let thumbnail = get_decendant_node!(entry, "thumbnail").attribute("url").unwrap_or("");
        let description = get_decendant_node!(entry, "description").text().unwrap_or("");
        let date = match DateTime::parse_from_rfc2822(video_published) {
            Ok(x) => x.to_rfc3339(),
            Err(_) => "2002-10-02T10:00:00-05:00".to_string(),
        };
        let content = get_decendant_node!(entry, "encoded").text().map(|x| x.to_string());
        Video { 
            channel: title.to_string(),
            title: video_title.to_string(),
            url: url.to_string(),
            published: date,
            description: description.to_string(),
            thumbnail: thumbnail.to_string(),
            content: content,
            flag: default_flag(),
        }
    }).collect::<Vec<Video>>()
}

fn get_channel_videos_from_contents(contents: &str) -> Vec<Video> {
    match roxmltree::Document::parse(contents) {
        Ok(document) =>
            match document.descendants().find(|n| n.tag_name().name() == "channel") {
                Some(channel) => get_peertube_channel_videos(channel),
                None => get_youtube_channel_videos(document),
            },
        Err(e) => {
            debug(&format!("failed parsing xml {}", e));
            vec![]
        },
    }
}

async fn get_channel_videos(client: &reqwest::Client, channel_url: String) -> Option<Vec<Video>> {
    for _i in 0..2 {
        let wrapped_response = client.get(channel_url.as_str()).header("Accept-Encoding", "gzip").send().await;
        match wrapped_response {
            Ok(response) =>
                if response.status().is_success() {
                    match response.text().await {
                        Ok(text) => return Some(get_channel_videos_from_contents(&text)),
                        Err(_) => { }
                    }
                },
            Err(_e) if _i == 1 => debug(&format!("failed loading {}: {}", &channel_url, _e)),
            Err(_) => {
            }
        }
    }
    return None
}

async fn get_videos(xml: String, additional_channel_ids: &[String], additional_channel_urls: &[String]) -> Vec<Option<Vec<Video>>> {
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
                    let futs : Vec<_> = urls_from_xml.iter().map(|url| get_channel_videos(&client, url.to_string())).collect();
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

fn to_show_videos(videos: &mut Vec<Video>, start: usize, end: usize, filter: &Regex) -> Vec<Video> {
    videos.sort_by(|a, b| b.published.cmp(&a.published));
    let filtered_videos = videos.iter().filter(|video| 
        filter.is_match(&video.title) || filter.is_match(&video.channel)
    ).cloned().collect::<Vec<Video>>();
    let new_end = std::cmp::min(end, filtered_videos.len());
    let mut result = filtered_videos[start..new_end].to_vec();
    result.reverse();
    result
}

fn save_videos(app_config: &AppConfig, videos: &Videos) {
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

async fn load(reload: bool, app_config: &AppConfig, original_videos: &Videos) -> Option<Videos> {
    match get_subscriptions_xml() {
        Ok(xml) => {
            let path = app_config.cache_path.as_str();
            if reload || fs::metadata(path).is_err() {
                let mut one_query_failed = false;
                let empty_vec = vec![];
                let vids = get_videos(xml, &app_config.channel_ids, &app_config.channel_urls).await
                    .iter().map(|x| 
                        match x.as_ref() {
                            Some(res) => res,
                            None => {
                                one_query_failed = true;
                                &empty_vec
                            }
                        }
                        ).flat_map(|x| x).cloned().collect::<Vec<Video>>();
                if one_query_failed {
                    return None
                }
                let mut videos = Videos { videos:  vids };
                
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
    let size = terminal_size();
    if let Some((Width(_), Height(h))) = size {
        (h - 1) as usize
    } else {
        20
    }
}

fn get_cols() -> usize {
    let size = terminal_size();
    if let Some((Width(w), Height(_))) = size {
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
    toshow: Vec<Video>,
    videos: Videos,
    app_config: AppConfig,
}

fn print_videos(toshow: &[Video]) {
    let cols = get_cols();
    let channel_max_size = cols / 3;
    let max = toshow.iter().fold(0, |acc, x| std::cmp::max(std::cmp::min(x.channel.chars().count(), channel_max_size), acc));
    for video in toshow {
        let published = video.published.split('T').collect::<Vec<&str>>();
        let whitespaces = " ".repeat(max - std::cmp::min(video.channel.chars().count(), channel_max_size));
        let channel_short = video.channel.chars().take(channel_max_size).collect::<String>();
        let s = format!("  {} \x1b[36m{}\x1b[0m \x1b[34m{}\x1b[0m{} {}",  flag_to_string(&video.flag), published[0][5..10].to_string(), channel_short, whitespaces, video.title);
        println!("{}", s.chars().take(min(s.chars().count(), cols-4+9+9+2)).collect::<String>());
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
                        for byte_result in stdout.bytes() {
                            match byte_result {
                                Ok(byte) => {
                                    print!("{}", byte as char);
                                    flush_stdout();
                                },
                                Err(_) => { }
                            }
                        },
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

fn play_video(path: &str, app_config: &AppConfig) {
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

fn play_url(url: &String, app_config: &AppConfig) {
    if app_config.mpv_mode && fs::metadata(&app_config.mpv_path).is_ok() {
        let message = format!("playing {} with mpv...", url);
        debug(&message);
        read_command_output(
            Command::new(&app_config.mpv_path)
            .arg("-fs")
            .arg("-really-quiet")
            .arg("--ytdl-format")
            .arg(&app_config.youtubedl_format)
            .arg(&url)
            , &app_config.mpv_path);
    } else {
        clear();
        let path = format!("{}/{}.{}", app_config.video_path, base64::encode(&url), app_config.video_extension);
        download_video(&path, &url, app_config);
        play_video(&path, app_config);
    }
}

fn play(v: &Video, app_config: &AppConfig) {
    play_url(&v.url, app_config);
}

fn print_help() {
    println!("\x1b[34;1m{}\x1b[0m", "youtube-subscriptions");
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

fn print_info(v: &Video) {
    println!("\x1b[34;1m{}\x1b[0m", v.title);
    println!();
    println!("from \x1b[36m{}\x1b[0m", v.channel);
    println!();
    println!("{}", v.description);
    match &v.content {
        Some(x) => {
            println!();
            let cols = get_cols();
            println!("{}", html2text::from_read(x.as_bytes(), cols));
        },
        None => {}
    }
}

fn quit() {
    show_cursor();
    rmcup();
}

impl YoutubeSubscribtions {

    fn clear_and_print_videos(&mut self) {
        clear();
        print_videos(&self.toshow)
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
        self.toshow = to_show_videos(&mut self.videos.videos, self.start, self.start + self.n, &self.filter);
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
        self.toshow = to_show_videos(&mut self.videos.videos, self.start, self.n, &self.filter);
    }

    fn play_current(&mut self) {
        if self.i < self.toshow.len() {
            play(&self.toshow[self.i], &self.app_config);
            self.flag(&Some(Flag::Read));
            self.clear_and_print_videos();
        }
    }

    fn display_current_thumbnail(&mut self) {
        if self.i < self.toshow.len() {
            let _res = webbrowser::open(&self.toshow[self.i].thumbnail);
            self.clear_and_print_videos();
        }
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
            if let "o" = s[0] { play_url(&s[1].to_string(), &self.app_config) }
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
            self.wait_key_press_and_clear_and_print_videos()
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
                                Char('T') => self.display_current_thumbnail(),
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
                videos: Videos{videos: vec![]},
                app_config: load_config(),
            };
        yts.run().await;
        },
        Err(_) => {
            println!("failed creating regex")
        }
    }
}
