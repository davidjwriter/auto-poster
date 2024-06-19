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
use reqwest;
use aws_sdk_sns::Client as SnsClient;
use aws_config::{meta::region::RegionProviderChain, SdkConfig};
use aws_sdk_dynamodb::{config::Region, meta::PKG_VERSION};
use aws_sdk_dynamodb::Client as DbClient;
use aws_sdk_dynamodb::types::{AttributeValue};
use aws_sdk_dynamodb::operation::get_item::GetItemInput;
use lambda_runtime::{LambdaEvent};
use std::fmt;
use std::error::Error as StdError;
use chrono::{Local, NaiveTime, DateTime, Timelike};



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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ContentType {
    POST,
    THREAD
}

pub trait SocialPost {
    fn get_post(self) -> String;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub uuid: String,
    pub post: String
}

impl SocialPost for Post {
    fn get_post(self) -> String {
        return self.post;
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Posts {
    pub posts: Vec<Post>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScheduledPost {
    pub uuid: String,
    pub post: String,
    pub time: String,
    pub recurring: bool,
}

impl SocialPost for ScheduledPost {
    fn get_post(self) -> String {
        return self.post;
    }
}

// Implement Display for the Failure response so that we can then implement Error.
impl std::fmt::Display for FailureResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.body)
    }
}

pub async fn make_config(opt: Opt) -> Result<SdkConfig, Error> {
    let region_provider = make_region_provider(opt.region);

    println!();
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

async fn get_scheduled_table_name() -> Option<String> {
    env::var("SCHEDULED_TABLE_NAME").ok()
}

async fn get_sns_arn() -> Option<String> {
    env::var("SNS_ARN").ok()
}

async fn delete_post_from_db(client: &DbClient, table_name: &str, uuid: String) -> Result<(), Error> {
    let pk = AttributeValue::S(uuid);

    client.delete_item()
        .table_name(table_name)
        .key("uuid".to_string(), pk)
        .send().await?;

    Ok(())
}

async fn check_scheduled_posts(client: &DbClient, table_name: &str) -> Result<Option<ScheduledPost>, Error> {
    let response = match client.scan()
        .table_name(table_name)
        .send().await {
            Ok(r) => r,
            Err(_) => return Ok(None)
        };

    let items = response.items.ok_or_else(|| MyError::new("No items found in response"))?;
    let mut posts: Vec<ScheduledPost> = Vec::new();

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

        let time: String = item
            .get("time")
            .ok_or_else(|| MyError::new("Missing 'time' attribute"))?
            .as_s()
            .or_else(|_| Err(MyError::new("Error getting time S attribute")))?
            .to_string();

        let recurring: bool = *item
            .get("recurring")
            .ok_or_else(|| MyError::new("Missing 'recurring' attribute"))?
            .as_bool()
            .or_else(|_| Err(MyError::new("Error getting recurring Bool attribute")))?;

        posts.push(ScheduledPost{post, uuid, time, recurring});
    }
    println!("Looking through all scheduled posts: {}", posts.len());
    let now = Local::now();
    let filtered_posts: Vec<ScheduledPost> = posts.into_iter()
        .filter(|post| {
            let scheduled_time: DateTime<Local> = match DateTime::from_str(&post.time) {
                Ok(t) => t,
                Err(_) => return false
            };
            println!("Comparing scheduled hour: {} to now: {}", scheduled_time.hour(), now.hour());
            scheduled_time.hour() == now.hour()
        })
        .collect::<Vec<ScheduledPost>>();
    let item = filtered_posts.first().cloned().ok_or_else(|| MyError::new("No Scheduled Posts"))?;
    
    Ok(Some(item))
}

async fn get_new_post_from_db(client: &DbClient, table_name: &str) -> Result<Post, Error> {
    let response = client.scan()
        .table_name(table_name)
        .limit(1)
        .send().await?;

    println!("DynamoDB Response: {:?}", response);
    let items = response.items.ok_or_else(|| MyError::new("No items found in response"))?;
    let item = items.first().ok_or_else(|| MyError::new("No items found in response"))?;

    let content: String = item
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

    Ok(Post {
        uuid: uuid,
        post: content
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = handler_fn(handler);
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn worker() -> Result<String, Error> {
    // 1. Create DB client
    let opt = Opt {
        region: Some("us-east-1".to_string()),
        verbose: true,
    };
    let config = match make_config(opt).await {
        Ok(c) => c,
        Err(e) => {
            return Ok(format!("Error making config: {}", e.to_string()));
            
        },
    };
    let db_client = DbClient::new(&config);
    let table_name = match get_table_name().await {
        Some(t) => t,
        None => {
            return Ok("TABLE_NAME not set".to_string());
        }
    };
    let scheduled_table_name: String = match get_scheduled_table_name().await {
        Some(t) => t,
        None => {
            return Ok("SCHEDULED_TABLE_NAME not set".to_string());
        }
    };

    // 2. Check Scheduled Table First
    let scheduled_post: Option<ScheduledPost> = match check_scheduled_posts(&db_client, &scheduled_table_name).await {
        Ok(s) => s,
        Err(e) => None
    };

    // 3. Get a new post from DB
    let mut message = String::new();
    let mut uuid_to_delete: Option<String> = None;
    let mut table_to_delete_from = &String::new();

    if let Some(s_post) = scheduled_post {
        println!("Sending a Scheduled Post");
        message = s_post.post;
        uuid_to_delete = match s_post.recurring {
            false => Some(s_post.uuid),
            true => None,
        };
        table_to_delete_from = &scheduled_table_name;
    } else {
        let post = match get_new_post_from_db(&db_client, &table_name).await {
            Ok(p) => p,
            Err(e) => return Ok(format!("Failed: {:?}", e)),
        };
        println!("Sending a normal post");
        message = post.post;
        uuid_to_delete = Some(post.uuid);
        table_to_delete_from = &table_name;
    }

    println!("Post: {:?}", message);

    // 4. Send to SNS
    let sns_arn = match get_sns_arn().await {
        Some(t) => t,
        None => {
            return Ok(format!("No SNS_ARN provided."));
        }
    };

    let sns_client = SnsClient::new(&config);
    match sns_client.publish()
        .topic_arn(sns_arn)
        .message_group_id(Uuid::new_v4().to_string())
        .message(serde_json::to_string(&message).unwrap())
        .send().await {
            Ok(output) => println!("Successfully send! {:?}", output),
            Err(e) => return Ok(format!("Failed :/ {:?}", e)),
        };
    println!("Published!");

    // 5. Delete post from DB
    if let Some(uuid) = uuid_to_delete {
        match delete_post_from_db(&db_client, table_to_delete_from, uuid).await {
            Ok(s) => return Ok(format!("Success!")),
            Err(e) => return Ok(format!("Failed :/ {:?}", e)),
        };
    } else {
        Ok(String::from("Success!"))
    }
}

async fn handler(_event: Value, _ctx: lambda_runtime::Context) -> Result<String, Error> {
    Ok(worker().await?)
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
        let resp = aw!(worker());
        println!("Response: {:?}", resp);
    }
}
