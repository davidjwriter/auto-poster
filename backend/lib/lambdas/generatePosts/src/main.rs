use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use futures_util::future::join_all;
use reqwest::get;
use select::document::Document;
use select::predicate::Name;
use uuid::Uuid;
use std::env;
use openai_api_rs::v1::api;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest};
use openai_api_rs::v1::image::ImageGenerationRequest;
use openai_api_rs::v1::error::APIError;
use scraper::{Html, Selector};
use lambda_http::{Response, Body, Error, Request};
use lambda_runtime::handler_fn;
use tokio::fs::File;
use tokio::time::Duration;
use tokio::fs::File as AsyncFile;
use tokio_util::codec::{BytesCodec, FramedRead};
use std::path::Path;
use std::io::prelude::*;
use base64;
use tokio::io::AsyncWriteExt;
use dotenv::dotenv;
use std::any::Any;
use std::str::FromStr;
use xml::reader::{EventReader, XmlEvent};
use regex::Regex;


const PROMPT: &str = "Create 12 powerful short Tweets that 
inspire conversation from this article. Respond with the 
Tweets in JSON format like this: {posts: [post: <str>]}";

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct FailureResponse {
    pub body: String,
}

type WorkerResponse = Result<SuccessResponse, FailureResponse>;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ContentType {
    POST,
    THREAD
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub post: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Posts {
    pub posts: Vec<Post>
}

// Implement Display for the Failure response so that we can then implement Error.
impl std::fmt::Display for FailureResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.body)
    }
}

// Implement Error for the FailureResponse so that we can `?` (try) the Response
// returned by `lambda_runtime::run(func).await` in `fn main`.
impl std::error::Error for FailureResponse {}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = handler_fn(handler);
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn cleanup(content: String) -> Result<String, FailureResponse> {
    // Remove xml tags
    let re = Regex::new(r"<[^>]*>").unwrap();
    let cleanup_string = re.replace_all(content.as_str(), "");

    // Remove articles
    let pattern = r"\b(?:a|an|the|this|that|these|those|it|he|she|they|them)\b";
    let re = Regex::new(pattern).unwrap();

    let final_string = re.replace_all(cleanup_string.as_ref(), "");
    Ok(final_string.to_string())
}

async fn get_current_newsletter_content(url: &str) -> Result<SuccessResponse, FailureResponse> {
    let tag_name = "item";
    // Send a GET request to the URL
    let response = match get(url).await {
        Ok(r) => r,
        Err(e) => {
            println!("Error reading URL: {:?} {:?}", url, e);
            return Err(FailureResponse {
                body: format!("Error reading URL: {}", e)
            });
        }
    };

    // Read the response body into a string
    let mut xml_content = match response.text().await {
        Ok(c) => c,
        Err(e) => {
            println!("Error reading URL contents: {:?}", e);
            return Err(FailureResponse {
                body: format!("Error reading URL contents: {}", e)
            });
        }
    };

    // Parse XML content
    let parser = EventReader::new(xml_content.as_bytes());
    let mut inside_tag = false;
    let mut result = String::new();
    
    for event in parser {
        match event {
            Ok(XmlEvent::StartElement { name, .. }) if name.local_name == tag_name => {
                inside_tag = true;
            }
            Ok(XmlEvent::EndElement { name }) if name.local_name == tag_name => {
                break;
            }
            Ok(XmlEvent::CData(text)) if inside_tag => {
                result.push_str(&text);
            }
            _ => {}
        }
    }
    
    return Ok(SuccessResponse {
        body: result
    });
}

async fn get_api_key() -> Option<String> {
    env::var("OPEN_AI_API_KEY").ok()
}

async fn generate_posts(contents: String) -> Result<Posts, FailureResponse> {
    let open_ai_api_key = get_api_key().await;
    if let Some(api_key) = open_ai_api_key {
        let client = api::Client::new(api_key);
        let req = ChatCompletionRequest {
            model: chat_completion::GPT4.to_string(),
            messages: vec![chat_completion::ChatCompletionMessage {
                role: chat_completion::MessageRole::user,
                content: format!("{} {}", PROMPT.to_string(), contents),
                name: None,
                function_call: None,
            }],
            functions: None,
            function_call: None,
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
        };
        println!("Chat Request: {:?}", req);
        let result = match client.chat_completion(req).await {
            Ok(r) => r,
            Err(e) => {
                println!("Error with OpenAI: {:?}", e);
                return Err(FailureResponse {
                    body: format!("Error getting response from OpenAI: {:?}", e)
                });
            }
        };
        let generated_content = match &result.choices[0].message.content {
            Some(c) => c,
            None => {
                println!("Could not get message content");
                return Err(FailureResponse {
                    body: format!("Could not get message content")
                })
            },
        };
        let content = match extract_json(&generated_content) {
            Some(s) => s,
            None => {
                println!("Error parsing posts conents!");
                return Err(FailureResponse {
                    body: format!("Error parsing posts contents!")
                });            
            },
        };
        println!("\n\nContent: {:?}\n\n", content);
        let posts: Posts = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                println!("Error parsing JSON {:?}", e);
                return Err(FailureResponse {
                    body: format!("Error parsing JSON {:?}", e)
                });
            }
        };
        return Ok(posts);
    }
    return Err(FailureResponse {
        body: String::from("API Key Not Set")
    });
}



async fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}


/**
 * Calls our add to db API
 */
pub async fn add_to_db(posts: Posts) -> Result<String, Error> {
    Ok(String::from("Ok"))
}

fn extract_json(json_string: &str) -> Option<String> {
    // Find the positions of the first opening and closing curly braces
    let start_pos = json_string.find('{');
    let end_pos = json_string.rfind('}');

    if let (Some(start), Some(end)) = (start_pos, end_pos) {
        // Extract the content between the curly braces, including the braces themselves
        let json_body = &json_string[start..=end];
        return Some(json_body.trim().to_string());
    }

    // If no match was found, return None
    None
}


async fn handler(_event: Value, _ctx: lambda_runtime::Context) -> Result<String, Error> {
    // 1. First retrieve the current contents of our newsletters

    // 2. Generate content

    // 3. Upload to the DB

    Ok("Success!".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn test_get_newsletter_content() {
        let url = "https://davidjmeyer.substack.com/feed";

        let response = aw!(get_current_newsletter_content(url)).unwrap().body;

        // println!("Response: {:?}", response);

        let cleanup = aw!(cleanup(response)).unwrap();
        println!("Response: {:?}", cleanup);
    }

    #[test]
    fn test_generate_posts() {
        dotenv::from_filename("../../.env").ok();
        let url = "https://davidjmeyer.substack.com/feed";
        let content = aw!(get_current_newsletter_content(url)).unwrap().body;
        let clean_content = aw!(cleanup(content)).unwrap();
        let posts = aw!(generate_posts(clean_content));
        println!("Posts: {:?}", posts);
    }

}
