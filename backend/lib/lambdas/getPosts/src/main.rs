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
use lambda_http::{service_fn, Response, Body, Error, Request};
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
use reqwest;
use aws_sdk_sns::Client as SnsClient;
use aws_config::{meta::region::RegionProviderChain, SdkConfig};
use aws_sdk_dynamodb::{config::Region, meta::PKG_VERSION};
use aws_sdk_dynamodb::Client as DbClient;
use aws_sdk_dynamodb::types::{AttributeValue};
use aws_sdk_dynamodb::operation::get_item::GetItemInput;
use std::fmt;
use std::error::Error as StdError;


#[derive(Debug)]
pub struct Opt {
    /// The AWS Region.
    pub region: Option<String>,
    /// Whether to display additional information.
    pub verbose: bool,
}

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct FailureResponse {
    pub body: String,
}

type WorkerResponse = Result<SuccessResponse, FailureResponse>;

// Error handling
#[derive(Debug)]
struct MyError {
    message: String,
}

impl StdError for MyError {}

impl MyError {
    fn new(message: &str) -> MyError {
        MyError {
            message: String::from(message)
        }
    }
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub uuid: String,
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

pub async fn make_config(opt: Opt) -> Result<SdkConfig, Error> {
    let region_provider = make_region_provider(opt.region);

    if opt.verbose {
        println!("DynamoDB client version: {}", PKG_VERSION);
        println!(
            "Region: {}",
            region_provider.region().await.unwrap().as_ref()
        );
        println!();
    }

    Ok(aws_config::from_env().region(region_provider).load().await)
}

pub fn make_region_provider(region: Option<String>) -> RegionProviderChain {
    RegionProviderChain::first_try(region.map(Region::new))
        .or_default_provider()
        .or_else(Region::new("us-east-1"))
}

async fn get_table_name() -> Option<String> {
    env::var("TABLE_NAME").ok()
}

async fn get_posts_from_db(client: &DbClient, table_name: &str) -> Result<Posts, Error> {
    let response = client.scan()
        .table_name(table_name)
        .send().await?;

    let items = response.items.ok_or_else(|| MyError::new("No items found in response"))?;
    let mut posts = Vec::new();
    for item in items {
        let post: String = item
            .get("post")
            .ok_or_else(|| MyError::new("Missing 'post' attribute"))?
            .as_s()
            .or_else(|_| Err(MyError::new("Error getting post S attribute")))?
            .to_string();

        let uuid: String = item
            .get("uuid")
            .ok_or_else(|| MyError::new("Missing 'uuid' attribute"))?
            .as_s()
            .or_else(|_| Err(MyError::new("Error getting uuid S attribute")))?
            .to_string();
        posts.push(Post {post, uuid});
    }

    Ok(Posts {
        posts: posts
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(handler);
    lambda_http::run(func).await?;

    Ok(())
}

async fn handler(_request: Request) -> Result<Response<String>, Error> {
    // 1. Create DB client
    let opt = Opt {
        region: Some("us-east-1".to_string()),
        verbose: true,
    };
    let config = match make_config(opt).await {
        Ok(c) => c,
        Err(e) => {
            return Ok(Response::builder()
                .status(500)
                .header("Access-Control-Allow-Origin", "*")
                .body(format!("Error making config: {}", e.to_string()))?)            
        },
    };
    let db_client = DbClient::new(&config);
    let table_name = match get_table_name().await {
        Some(t) => t,
        None => {
            return Ok(Response::builder()
            .status(500)
            .header("Access-Control-Allow-Origin", "*")
            .body(String::from("No Table Name Set"))?)
        }
    };
    // 2. Get a new post from DB
    let posts = match get_posts_from_db(&db_client, &table_name).await {
        Ok(p) => p,
        Err(e) => return Ok(Response::builder()
        .status(500)
        .header("Access-Control-Allow-Origin", "*")
        .body(format!("Get Posts Internal Error: {}", e.to_string()))?)
    };

    Ok(Response::builder()
        .status(200)
        .header("Access-Control-Allow-Origin", "*")
        .body(serde_json::to_string(&posts).unwrap())?
    )
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
    fn test_send_posts() {
        dotenv::from_filename("../.env").ok();
        for var in dotenv::vars() {
            println!("{:?}", var);
        }
        let resp = aw!(handler(Request::new(Body::Empty))).unwrap();
        let posts: Posts = serde_json::from_str(resp.body()).unwrap();
        println!("Total Posts: {}", posts.posts.len());
        for post in posts.posts {
            println!("* UUID: {} Post: {}", post.uuid, post.post);
        }
    }
}
