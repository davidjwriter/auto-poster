use deso_sdk;
use lambda_http::{service_fn, Body, Error, Request, Response};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::env;

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub post: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(handler);
    lambda_http::run(func).await?;

    Ok(())
}

async fn get_deso_user() -> Option<String> {
    env::var("DESO_USER").ok()
}

async fn get_deso_private_key() -> Option<String> {
    env::var("DESO_PRIVATE_KEY").ok()
}

async fn worker(body: &str) -> Result<String, Error> {
    let post: Post = match serde_json::from_str(&body) {
        Ok(u) => u,
        Err(e) => {
            println!("Error matching URL: {:?}", e);
            return Err(FailureResponse {
                body: format!("Error matching URL: {:?}", e),
            });
        }
    };

    let body = post.post;
    println!("Body: {:?}", body);
    let comment = String::from("If you like this kind of content, make sure to checkout my newsletter and remember, run with joy! https://davidjmeyer.substack.com");

    let deso_account = deso_sdk::DesoAccountBuilder::new()
        .public_key(get_deso_user())
        .seed_hex_key(get_deso_private_key())
        .build()
        .unwrap();

    let post_data = deso_sdk::SubmitPostDataBuilder::new()
        .body(body)
        .public_key(get_deso_user())
        .build()
        .unwrap();

    let post_hash_hex = deso_sdk::create_post(&deso_account, &post_data)
        .await
        .unwrap()
        .post_entry_response
        .post_hash_hex;

    let comment_data = deso_sdk::SubmitPostDataBuilder::new()
        .body(comment)
        .parent_post_hash_hex(post_hash_hex)
        .public_key(get_deso_user())
        .build()
        .unwrap();

    deso_sdk::create_post(&deso_account, &comment_d).await?;

    Ok(String::from("Success!"))
}

async fn handler(event: LambdaEvent<sns::SnsEvent>) -> Result<String, Error> {
    // 1. Get SNS event records
    let records = event.payload.records;

    // 2. iterate through records and call worker function
    let mut messages: Vec<String> = Vec::new();
    for record in records {
        let message = match worker(&record.sns.message).await {
            Ok(s) => s.body,
            Err(e) => e.body,
        };
        messages.push(message);
    }

    Ok(json!(messages))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }
}
