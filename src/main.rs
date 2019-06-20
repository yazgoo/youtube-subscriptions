extern crate sxd_document;
extern crate sxd_xpath;
extern crate dirs;
extern crate reqwest;
extern crate serde_json;
extern crate terminal_size;
extern crate crossterm_input;
extern crate crossterm;
extern crate par_map;


use serde::{Serialize, Deserialize};
use sxd_document::parser;
use sxd_xpath::{evaluate_xpath, Value, Factory};
use sxd_xpath::context::Context;
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::io::Error;
use sxd_document::dom::Element;
use terminal_size::{Width, Height, terminal_size};
use std::cmp::min;
use std::process::{Command, Stdio};
use crossterm_input::{input, RawScreen};
use crossterm::{terminal, ClearType};
use par_map::ParMap;

fn get_subscriptions_xml() -> Result<String, Error> {
    match dirs::home_dir() {
        Some(home) =>
            match home.to_str() {
                Some(s) =>
                    return fs::read_to_string(format!("{}/.config/youtube-subscriptions/subscription_manager", s)),
                None =>
                    panic!("failed reading subscription_manager")
            },
        None =>
            panic!("failed reading subscription_manager")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Video {
    channel: String,
    title: String,
    thumbnail: String,
    url: String,
    published: String,
    description: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Videos {
    videos: Vec<Video>,
}

fn get_value(xpath: String, node: Element) -> String {
    let factory = Factory::new();
    let xpath = factory.build(xpath.as_str()).expect("Could not compile XPath");
    let xpath = xpath.expect("No XPath was compiled");
    let context = Context::new();
    return xpath.evaluate(&context, node).unwrap_or(Value::String("".to_string())).string().to_string();
}

fn get_channel_videos(channel_url: String) -> Vec<Video> {
    match reqwest::get(channel_url.replace("https:", "http:").as_str()) {
        Ok(mut result) => 
            match result.text() {
                Ok(contents) => {
                    let package = parser::parse(contents.as_str()).expect("failed to parse XML");
                    let document = package.as_document();
                    let title = evaluate_xpath(&document, "string(/*[local-name() = 'feed']/*[local-name() = 'title']/text())").unwrap_or(Value::String("".to_string())).string();
                    match evaluate_xpath(&document, "/*[local-name() = 'feed']/*[local-name() = 'entry']") {
                        Ok(val) => {
                            if let Value::Nodeset(entries) = val {
                                entries.iter().flat_map( |entry|
                                     match entry.element() {
                                         Some(_element) => 
                                         {
                                             vec![Video { 
                                                 channel: title.to_string(),
                                                 title: get_value("string(*[local-name() = 'title']/text())".to_string(), _element),
                                                 thumbnail: get_value("string(*[local-name() = 'group']/*[local-name() = 'thumbnail']/@url)".to_string(), _element),
                                                 url: get_value("string(*[local-name() = 'group']/*[local-name() = 'content']/@url)".to_string(), _element),
                                                 published: get_value("string(*[local-name() = 'published']/text())".to_string(), _element),
                                                 description: get_value("string(*[local-name() = 'group']/*[local-name() = 'description']/text())".to_string(), _element),
                                             }]
                                         },
                                         None => vec![]
                                         }
                                ).collect()
                            }
                            else {
                                vec![]
                            }
                        },
                        Err(_) => {
                            println!("aaaaa");
                            vec![]
                        }
                    }
                },
                Err(_) => {
                    println!("bbbbb");
                    vec![]
                },
            },
        Err(e) => {
            println!("{}", e);
            vec![]
        },
    }
}

fn print_animation(i: usize) -> usize {
    let animation = vec!['◜', '◝', '◞', '◟'];
    let ni = i % animation.len();
    print!("\r{}", animation[ni]);
    io::stdout().flush().unwrap();
    ni + 1
}

fn get_videos(xml: String) -> Vec<Video> {
    let package = parser::parse(xml.as_str()).expect("failed to parse XML");
    let document = package.as_document();
    let mut i = 0;
    match evaluate_xpath(&document, "//outline/@xmlUrl") {
        Ok(value) =>  {
            if let Value::Nodeset(urls) = value {
                urls.iter().flat_map( |url| {
                    i = print_animation(i);
                    match url.attribute() {
                        Some(attribute) => Some(attribute.value().to_string()),
                        None => None
                    }
                }
                ).par_flat_map( |url|
                       get_channel_videos(url)
                ).collect()
            }
            else {
                vec![]
            }
        },
        Err(err) => {
            println!("{:?}", err);
            vec![]
        }
    }
    
}

fn to_show_videos(videos: &mut Vec<Video>, start: usize, count: usize) -> Vec<Video> {
    videos.sort_by(|a, b| b.published.cmp(&a.published));
    let mut result = videos[start..count].to_vec();
    result.reverse();
    return result;
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

fn hide_cursor() {
    print!("\x1b[?25l");
    io::stdout().flush().unwrap();
}

fn clear() {
    let terminal = terminal();
    terminal.clear(ClearType::All);
}

fn show_cursor() {
    print!("\x1b[?25h");
    io::stdout().flush().unwrap();
}

fn move_cursor(i: usize) {
    print!("\x1b[{};0f", i + 1);
    io::stdout().flush().unwrap();
}

fn print_selector(i: usize) {
    move_cursor(i);
    print!("┃");
    io::stdout().flush().unwrap();
}

fn clear_selector(i: usize) {
    move_cursor(i);
    print!(" ");
    io::stdout().flush().unwrap();
}

fn jump(i: usize, new_i: usize) -> usize {
    clear_selector(i);
    return new_i;
}

struct YoutubeSubscribtions {
    n: usize,
    start: usize,
    i: usize,
    toshow: Vec<Video>,
    videos: Videos
}

fn print_videos(toshow: &Vec<Video>) {
    let max = toshow.iter().fold(0, |acc, x| if x.channel.chars().count() > acc { x.channel.chars().count() } else { acc } );
    let cols = get_cols();
    for video in toshow {
        let published = video.published.split("T").collect::<Vec<&str>>();
        let whitespaces = " ".repeat(max - video.channel.chars().count());
        let s = format!("  {} {}{} {}", published[0][5..10].to_string(), video.channel, whitespaces, video.title);
        println!("{}", s[0..min(s.len(), cols-4)].to_string());
    }
}

fn get_id(v: &Video) -> Option<Option<String>> {
    v.url.split("/").collect::<Vec<&str>>().last().map( |page|
                                                        page.split("?").collect::<Vec<&str>>().first().map( |s| s.to_string() ))
}

fn run_vlc(binary: &str, path: &String) {
    let mut child1 = Command::new(&binary)
        .arg("--play-and-exit")
        .arg("-f")
        .arg(path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child1.wait();
}

fn play_video(path: &String) {
    let omxplayer_path = "/usr/bin/omxplayer";
    if fs::metadata(&omxplayer_path).is_ok() {
        let mut child1 = Command::new(omxplayer_path)
            .arg("-o")
            .arg("local")
            .arg(path)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child1.wait();
    }
    else {
        let macos_vlc = "/Applications/VLC.app/Contents/MacOS/VLC";
        if fs::metadata(&macos_vlc).is_ok() {
            run_vlc(&macos_vlc, &path);
        }
        else {
            let vlc = "vlc";
            run_vlc(&vlc, &path);
        }
    }
}

fn download_video(path: &String, id: &String) {
    if !fs::metadata(&path).is_ok() {
        let mut child0 = Command::new("youtube-dl")
            .arg("-f")
            .arg("[height <=? 360][ext = mp4]")
            .arg("-o")
            .arg(&path)
            .arg("--")
            .arg(&id)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child0.wait();
    }
}

fn play(v: &Video) {
    let id = get_id(v);
    match id {
        Some(Some(id)) => {
            let path = format!("/tmp/{}.mp4", id);
            download_video(&path, &id);
            play_video(&path);
            ()
        },
        _ => (),
    }
}

fn print_help(v: &Video) {
    println!("
  youtube-subscriptions: a tool to view your youtube subscriptions in a terminal

  q        quit
  j,down   move down
  k,up     move up
  g        go to top
  G        go to bottom
  r        soft refresh
  R        full refresh
  h        prints this help
  p,enter  plays video
  ")
}

fn print_info(v: &Video) {
    println!("{}", v.title);
    println!("");
    println!("from {}", v.channel);
    println!("");
    println!("{}", v.description);
}

impl YoutubeSubscribtions {

    fn clear_and_print_videos(&mut self) {
        clear();
        print_videos(&self.toshow)
    }

    fn soft_reload(&mut self) {
        self.n = get_lines();
        self.start = 0;
        self.toshow = to_show_videos(&mut self.videos.videos, self.start, self.start + self.n);
        self.i = 0;
        self.clear_and_print_videos()
    }

    fn first_page(&mut self) {
        self.n = get_lines();
        self.toshow = to_show_videos(&mut self.videos.videos, self.start, self.n);
    }

    fn play_current(&mut self) {
        clear();
        play(&self.toshow[self.i]);
        clear();
        self.soft_reload();
    }

    fn wait_key_press_and_soft_reload(&mut self) {
        {
            let input = input();
            let screen = RawScreen::into_raw_mode();
            input.read_char();
        }
        clear();
        self.soft_reload();
    }

    fn info(&mut self) {
        clear();
        print_info(&self.toshow[self.i]);
        self.wait_key_press_and_soft_reload()
    }

    fn help(&mut self) {
        clear();
        print_help(&self.toshow[self.i]);
        self.wait_key_press_and_soft_reload()
    }

    fn load(&mut self, reload: bool) -> Option<Videos> {
        match get_subscriptions_xml() {
            Ok(xml) => {
                let path = "/tmp/yts.json";
                if reload || !fs::metadata(path).is_ok() {
                    let videos = Videos { videos: get_videos(xml)};
                    let serialized = serde_json::to_string(&videos).unwrap();
                    fs::write(path, serialized); 
                }
                match fs::read_to_string(path) {
                    Ok(s) => 
                        Some(serde_json::from_str(s.as_str()).unwrap()),
                    Err(e) =>
                        None
                }
            },
            Err(e) =>
                None
        }
    }

    fn run(&mut self) {
                    self.videos = self.load(false).unwrap();
                    self.start = 0;
                    self.i = 0;
                    self.first_page();
                    self.clear_and_print_videos();
                    hide_cursor();
                    while true {
                        print_selector(self.i);
                        let input = input();
                        let result;
                        {
                            let screen = RawScreen::into_raw_mode();
                            result = input.read_char();
                        }
                        match result {
                            Ok(c) => {
                                match c {
                                    'q' => {
                                        show_cursor();
                                        break;
                                    },
                                    'j' | 'l' => self.i = jump(self.i, self.i + 1),

                                    'k' => 
                                        self.i = jump(self.i, 
                                                 if self.i > 0 { self.i - 1 } else { self.n - 1 }),
                                    'g' | 'H' => self.i = jump(self.i, 0),
                                    'M' => self.i = jump(self.i, self.n / 2),
                                    'G' | 'L' => self.i = jump(self.i, self.n - 1),
                                    'r' | '$' => self.soft_reload(),
                                    'R' => {self.videos = self.load(true).unwrap(); self.soft_reload()},
                                    'h' | '?' => self.help(),
                                    'i' => self.info(),
                                    'p' => self.play_current(),
                                    _ => ()
                                }
                            }
                            Err(_) => (),
                        };
                        self.i = self.i % self.n;
                    };
            }
}

fn main() {
    YoutubeSubscribtions{
        n: 0,
        start: 0,
        i: 0,
        toshow: vec![],
        videos: Videos{videos: vec![]},
    }.run();
}
