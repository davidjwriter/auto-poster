use serde::Deserialize;
use serde::Serialize;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use aws_lambda_events::event::sqs::SqsEvent;
use serde_json::json;
use std::env;
use reqwest;
use oauth1::Token;
use std::collections::HashMap;
use dotenv::dotenv;
use oauth1_header::{Credentials};
use oauth1_header::http::Method;

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub uuid: String,
    pub post: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tweet {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TweetData {
    pub edit_history_tweet_ids: Vec<String>,
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TweetResponse {
    pub data: TweetData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TweetComment {
    pub text: String,
    pub reply: Reply
}

impl TweetComment {
    pub fn new(comment: String, id: String) -> TweetComment {
        TweetComment {
            text: comment,
            reply: Reply {
                in_reply_to_tweet_id: id
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Reply {
    pub in_reply_to_tweet_id: String
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(handler);
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn get_consumer_key() -> Option<String> {
    env::var("CONSUMER_KEY").ok()
}

async fn get_consumer_secret() -> Option<String> {
    env::var("CONSUMER_SECRET").ok()
}

async fn get_access_token() -> Option<String> {
    env::var("ACCESS_TOKEN").ok()
}

async fn get_access_secret() -> Option<String> {
    env::var("ACCESS_TOKEN_SECRET").ok()
}

async fn worker(body: &str) -> Result<String, Error> {
    let post: Post = serde_json::from_str(&body).expect("Couldn't parse json post");

    let body = post.post;
    println!("Body: {:?}", body);

    let uri = "https://api.twitter.com/2/tweets";
    let consumer_key = get_consumer_key().await.expect("Missing Consumer Key");
    let consumer_secret = get_consumer_secret().await.expect("Missing Consumer Secret");
    let access_token = get_access_token().await.expect("Missing Access Token");
    let access_secret = get_access_secret().await.expect("Missing Access Secret");
    println!("Consumer Key and Secret: {}\n{}\nAccess token and secret: {}\n{}", consumer_key, consumer_secret, access_token, access_secret);
    let params = HashMap::new();
    
    let credentials = Credentials::new(
        &consumer_key,
        &consumer_secret,
        &access_token,
        &access_secret,
    );
    let header_value = credentials.auth(&Method::POST, uri, &params);
    let client = reqwest::Client::new();

    // Construct JSON body
    let json_body = json!({
        "text": body,
    });

    // Make the POST request with the authorization header
    let response = client.post(uri)
        .header("Authorization", &header_value)
        .header("Content-Type", "application/json")
        .json(&json_body)
        .send().await;

    // Make the comment
    let comment = String::from("If you like this kind of content, make sure to checkout my newsletter and remember, run with joy! https://davidjmeyer.substack.com");

    let raw_resp = response.unwrap().text().await.unwrap();
    let tweet_data: TweetResponse = serde_json::from_str(&raw_resp).expect("Error getting tweet data");
    println!("Tweet Data: {:?}", tweet_data);

    let comment_body = TweetComment::new(comment, tweet_data.data.id);

    let response = client.post(uri)
        .header("Authorization", &header_value)
        .header("Content-Type", "application/json")
        .json(&comment_body)
        .send().await;

    let raw_resp = response.unwrap().text().await.unwrap();
    println!("{}", raw_resp);

    Ok(String::from("Success"))

}

async fn handler(event: LambdaEvent<SqsEvent>) -> Result<String, Error> {
    // 1. Get SNS event records
    let records = event.payload.records;

    // 2. iterate through records and call worker function
    let mut messages = Vec::new();

    for record in records {
        let message = match worker(&record.body.unwrap()).await {
            Ok(s) => s,
            Err(e) => {
                println!("{:?}", e);
                String::new()
            },
        };
        messages.push(message);
    }

    Ok(serde_json::to_string(&messages).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lambda_events::event::sqs::{SqsEvent, SqsMessage};
    use lambda_runtime::LambdaEvent;
    use std::{collections::HashMap, hash::Hash};
    use chrono::{DateTime, Utc};
    // - message_id
    // - receipt_handle
    // - body
    // - md5_of_body
    // - md5_of_message_attributes
    // - attributes
    // - message_attributes
    // - event_source_arn
    // - event_source
    // - aws_region
    fn mock_sqs_event(message: &str) -> SqsEvent {
        SqsEvent {
            records: vec![SqsMessage {
                message_id: Some("EXAMPLE".to_string()),
                receipt_handle: Some("Receipt".to_string()),
                body: Some(message.to_string()),
                md5_of_body: None,
                md5_of_message_attributes: None,
                attributes: HashMap::new(),
                message_attributes: HashMap::new(),
                event_source_arn: None,
                event_source: None,
                aws_region: None
            }
            ]
        }
    }

    #[tokio::test]
    async fn test_handler() {
        dotenv().ok();
        let message = r#"{"post":"Thank you Lord for this day, may it be used for your glory! Gm everyone!", "uuid": "hello"}"#;
        let event = mock_sqs_event(message);
        let lambda_event = LambdaEvent {
            payload: event,
            context: lambda_runtime::Context::default(),
        };

        let result = handler(lambda_event).await;
        assert!(result.is_ok());
        let result_str = result.unwrap();
        assert!(result_str.contains("Success"));
    }
}