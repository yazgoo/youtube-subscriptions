extern crate sxd_document;
extern crate sxd_xpath;
extern crate dirs;
extern crate reqwest;

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

#[derive(Debug)]
struct Video {
    channel: String,
    title: String,
    thumbnail: String,
    url: String,
    published: String,
    description: String,
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

fn get_videos(channel_url: String) -> Vec<Video> {
    match reqwest::get(channel_url.as_str()) {
        Ok(mut result) => 
            match result.text() {
                Ok(contents) => {
                    let package = parser::parse(contents.as_str()).expect("failed to parse XML");
                    let document = package.as_document();
                    let title = evaluate_xpath(&document, "string(/*[local-name() = 'feed']/*[local-name() = 'title']/text())").unwrap_or(Value::String("".to_string())).string();
                    println!("{:?}", channel_url);
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
                                         None => vec![Video { 
                                                 channel: "a".to_string(),
                                                 title: "".to_string(),
                                                 thumbnail: "".to_string(),
                                                 url: "".to_string(),
                                                 published: "".to_string(),
                                                 description: "".to_string(),
                                             }]
                                         }
                                ).collect()
                            }
                            else {
                                vec![Video { 
                                                 channel: "b".to_string(),
                                                 title: "".to_string(),
                                                 thumbnail: "".to_string(),
                                                 url: "".to_string(),
                                                 published: "".to_string(),
                                                 description: "".to_string(),
                                             }]
                            }
                        },
                        Err(e) => {
                            println!("{}", e);
                            vec![Video { 
                                                 channel: "c".to_string(),
                                                 title: "".to_string(),
                                                 thumbnail: "".to_string(),
                                                 url: "".to_string(),
                                                 published: "".to_string(),
                                                 description: "".to_string(),
                                             }]
                        }
                    }
                },
                Err(_) =>
                            vec![Video { 
                                                 channel: "d".to_string(),
                                                 title: "".to_string(),
                                                 thumbnail: "".to_string(),
                                                 url: "".to_string(),
                                                 published: "".to_string(),
                                                 description: "".to_string(),
                                             }],
            },
        Err(_) =>
                            vec![Video { 
                                                 channel: "e".to_string(),
                                                 title: "".to_string(),
                                                 thumbnail: "".to_string(),
                                                 url: "".to_string(),
                                                 published: "".to_string(),
                                                 description: "".to_string(),
                                             }],
    }
}

fn get_channels(xml: String) -> Vec<Video> {
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
                       get_videos(url)
                ).collect()
            }
            else {
                vec![]
            }
        Err(_) =>
            vec![]
    }
    
}

fn main() {
    match get_subscriptions_xml() {
        Ok(xml) =>
            println!("{:?}", get_channels(xml)),
        Err(e) =>
            panic!("error parsing header: {:?}", e)
    }
}
