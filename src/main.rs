extern crate sxd_document;
extern crate sxd_xpath;
extern crate dirs;
extern crate reqwest;
extern crate serde_json;

use serde::{Serialize, Deserialize};
use sxd_document::parser;
use sxd_xpath::{evaluate_xpath, Value, Factory};
use sxd_xpath::context::Context;
use std::fs;
use std::io::Error;
use sxd_document::dom::Element;

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
    return videos[start..count].to_vec();
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
                    let deserialized: Videos = serde_json::from_str(s.as_str()).unwrap();
                    println!("{:?}", to_show_videos(deserialized.videos, 0, 2));
                },
                Err(e) =>
                    println!("{}", e),
            }
        },
        Err(e) =>
            panic!("error parsing header: {:?}", e)
    }
}
