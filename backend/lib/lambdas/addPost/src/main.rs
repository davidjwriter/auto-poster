use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use lambda_runtime::{LambdaEvent};
use std::collections::HashMap;
use aws_sdk_dynamodb::types::{AttributeValue, WriteRequest, PutRequest};
use aws_sdk_dynamodb::operation::get_item::GetItemInput;
use aws_sdk_dynamodb::operation::put_item::PutItem;
use std::env;
use aws_config::{meta::region::RegionProviderChain, SdkConfig};
use aws_sdk_dynamodb::{config::Region, meta::PKG_VERSION};
use aws_sdk_dynamodb::Client as DbClient;
use std::iter::Iterator;
use uuid::Uuid;
use lambda_http::{service_fn, Response, Body, Error, Request};
use serde_json::json;


#[derive(Debug)]
pub struct Opt {
    /// The AWS Region.
    pub region: Option<String>,
    /// Whether to display additional information.
    pub verbose: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub post: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Posts {
    pub posts: Vec<Post>
}

// Define an iterator type for Posts
pub struct PostsIterator {
    inner: std::vec::IntoIter<Post>,
}

impl Iterator for PostsIterator {
    type Item = Post;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

// Implement IntoIterator for Posts
impl IntoIterator for Posts {
    type Item = Post;
    type IntoIter = PostsIterator;

    fn into_iter(self) -> Self::IntoIter {
        PostsIterator {
            inner: self.posts.into_iter(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(handler);
    lambda_http::run(func).await?;

    Ok(())
}

async fn get_table_name() -> Option<String> {
    env::var("TABLE_NAME").ok()
}

async fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

pub async fn make_config(opt: Opt) -> Result<SdkConfig, Error> {
    let region_provider = make_region_provider(opt.region);

    println!();
    if opt.verbose {
        println!("DynamoDB client version: {}", PKG_VERSION);
        println!(
            "Region:                  {}",
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


/**
 * Data format:
 * primary_key: uuid
 * name: string
 * ingredients: []
 * instructions: []
 * notes: string
 */
pub async fn add_to_db(client: &DbClient, posts: Posts, table: String) -> Result<String, Error> {
    // Create a vector to hold the write requests for batch upload
    let mut write_requests: Vec<WriteRequest> = Vec::new();

    // Iterate over each post and create a write request for each
    for post in posts {
        let uuid = AttributeValue::S(generate_uuid().await);
        let mut item = HashMap::new();
        item.insert("uuid".to_string(), uuid);
        item.insert("post".to_string(), AttributeValue::S(post.post));
        let put_request = PutRequest::builder().set_item(Some(item)).build();
        write_requests.push(WriteRequest::builder()
            .put_request(put_request).build());
    }

    // Split write requests into batches of maximum 25 items
    let mut batch_write_requests: Vec<Vec<WriteRequest>> = Vec::new();
    for chunk in write_requests.chunks(25) {
        batch_write_requests.push(chunk.to_vec());
    }

    // Execute batch write requests
    for request_items in batch_write_requests {
        let request = client.batch_write_item().request_items(table.clone(), request_items).send().await;
        println!("Response: {:?}", request);
    }

    Ok(String::from("Posts added successfully"))
}


async fn handler(request: Request) -> Result<Response<String>, Error> {
    // 1. Create db client and get table name from env
    let opt = Opt {
        region: Some("us-east-1".to_string()),
        verbose: true,
    };
    let config = match make_config(opt).await {
        Ok(c) => c,
        Err(e) => {
            return Ok(Response::builder()
            .status(500)
            .body(format!("Error making config: {}", e.to_string()))?);
            
        },
    };
    let db_client = DbClient::new(&config);
    let table_name = match get_table_name().await {
        Some(t) => t,
        None => {
            return Ok(Response::builder()
            .status(500)
            .body(String::from("TABLE_NAME not set"))?);
        }
    };
    let body = request.body();
    let posts: Posts = serde_json::from_slice(&body)?;
    add_to_db(&db_client, posts, table_name);
    Ok(Response::builder()
        .status(200)
        .header("Access-Control-Allow-Origin", "*")
        .body(String::from("Success"))?)
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
    fn test_add_to_db() {
        let post = Post {
            post: String::from("Test Post 1")
        };
        let post2 = Post {
            post: String::from("Test Post 2")
        };
        let posts: Posts = Posts {
            posts: vec![post, post2]
        };
        let opt = Opt {
            region: Some("us-east-1".to_string()),
            verbose: true,
        };
        let config = aw!(make_config(opt)).unwrap();
        println!("{:?}", config);
        let db_client = DbClient::new(&config);
        let table_name = String::from("Posts");
        aw!(add_to_db(&db_client, posts, table_name));
    }
}