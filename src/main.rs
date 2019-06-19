extern crate sxd_document;
extern crate sxd_xpath;
extern crate dirs;
extern crate reqwest;
extern crate serde_json;
extern crate terminal_size;
extern crate crossterm_input;


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
use crossterm_input::{input, RawScreen};

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

/*
impl<'data, T: Sync + 'data> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}
*/

fn get_value(xpath: String, node: Element) -> String {
    let factory = Factory::new();
    let xpath = factory.build(xpath.as_str()).expect("Could not compile XPath");
    let xpath = xpath.expect("No XPath was compiled");
    let context = Context::new();
    return xpath.evaluate(&context, node).unwrap_or(Value::String("".to_string())).string().to_string();
}

fn get_channel_videos(channel_url: String) -> Vec<Video> {
    match reqwest::get(channel_url.as_str()) {
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
                            vec![]
                        }
                    }
                },
                Err(_) =>
                            vec![],
            },
        Err(_) =>
                            vec![],
    }
}

fn get_videos(xml: String) -> Vec<Video> {
    let package = parser::parse(xml.as_str()).expect("failed to parse XML");
    let document = package.as_document();
    match evaluate_xpath(&document, "//outline/@xmlUrl") {
        Ok(value) => 
            if let Value::Nodeset(urls) = value {
                urls.iter().flat_map( |url|
                    match url.attribute() {
                        Some(attribute) => Some(attribute.value().to_string()),
                        None => None
                    }
                ).flat_map( |url|
                       get_channel_videos(url)
                ).collect()
            }
            else {
                vec![]
            }
        Err(_) =>
            vec![]
    }
    
}

fn to_show_videos(mut videos: Vec<Video>, start: usize, count: usize) -> Vec<Video> {
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

fn print_videos(toshow: Vec<Video>) {
    let max = toshow.iter().fold(0, |acc, x| if x.channel.len() > acc { x.channel.len() } else { acc } );
    let cols = get_cols();
    for video in toshow {
        let published = video.published.split("T").collect::<Vec<&str>>();
        let whitespaces = " ".repeat(max - video.channel.len());
        let s = format!("  {} {}{} {}", published[0][5..10].to_string(), video.channel, whitespaces, video.title);
        println!("{}", s[0..min(s.len(), cols-4)].to_string())
    }
}

fn hide_cursor() {
    print!("\x1b[?25l");
    io::stdout().flush().unwrap();
}

fn show_cursor() {
    print!("\x1b[?25h");
    io::stdout().flush().unwrap();
}

fn move_cursor(i: usize) {
    print!("\x1b[{};0f", i);
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

fn main() {
    match get_subscriptions_xml() {
        Ok(xml) => {
            let path = "/tmp/yts.json";
            if !fs::metadata(path).is_ok() {
                let videos = Videos { videos: get_videos(xml)};
                let serialized = serde_json::to_string(&videos).unwrap();
                fs::write(path, serialized); 
            }
            match fs::read_to_string(path) {
                Ok(s) => {
                    let mut n = get_lines();
                    let deserialized: Videos = serde_json::from_str(s.as_str()).unwrap();
                    let mut i = 0;
                    let mut toshow = to_show_videos(deserialized.videos, 0, n);
                    print_videos(toshow);
                    hide_cursor();
                    let screen = RawScreen::into_raw_mode();
                    while true {
                        print_selector(i);
                        let input = input();
                        match input.read_char() {
                            Ok(c) => {
                                match c {
                                    'q' => {
                                        break;
                                        show_cursor();
                                    },
                                    'j' | 'l' => i = jump(i, i + 1),
                                    'k' | 'h' => i = jump(i, i - 1),
                                    _ => ()
                                }
                            }
                            Err(_) => (),
                        }
                        if i <= 0 {
                            i = n - 1;
                        } else {
                            i = i % n;
                        }
                    }
                },
                Err(e) =>
                    println!("{}", e),
            }
        },
        Err(e) =>
            panic!("error parsing header: {:?}", e)
    }
}
