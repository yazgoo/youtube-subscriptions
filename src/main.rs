extern crate sxd_document;
extern crate sxd_xpath;
extern crate dirs;

use sxd_document::parser;
use sxd_xpath::{evaluate_xpath, Value};
use std::fs;
use std::io::Error;

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

fn get_channels(xml: String) -> Vec<String> {
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
                ).collect()
            }
            else {
                vec![]
            }
        Err(e) =>
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
