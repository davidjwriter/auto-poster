use serde::Deserialize;
use serde::Serialize;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use aws_lambda_events::event::sns::SnsEvent;
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
    dotenv().ok();
    let post: Post = serde_json::from_str(&body).expect("Couldn't parse json post");

    let body = post.post;
    println!("Body: {:?}", body);
    let comment = String::from("If you like this kind of content, make sure to checkout my newsletter and remember, run with joy! https://davidjmeyer.substack.com");

    let uri = "https://api.twitter.com/2/tweets";
    let consumer_key = get_consumer_key().await.expect("Missing Consumer Key");
    let consumer_secret = get_consumer_secret().await.expect("Missing Consumer Secret");
    let access_token = get_access_token().await.expect("Missing Access Token");
    let access_secret = get_access_secret().await.expect("Missing Access Secret");

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
        .header("Authorization", header_value)
        .header("Content-Type", "application/json")
        .json(&json_body)
        .send().await;

    println!("Response: {:?}", response);

    Ok(String::from("Success"))

}

async fn handler(event: LambdaEvent<SnsEvent>) -> Result<String, Error> {
    // 1. Get SNS event records
    let records = event.payload.records;

    // 2. iterate through records and call worker function
    let mut messages = Vec::new();

    for record in records {
        let message = match worker(&record.sns.message).await {
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
    use aws_lambda_events::event::sns::{SnsEvent, SnsMessage, SnsRecord};
    use lambda_runtime::LambdaEvent;
    use std::collections::HashMap;
    use chrono::{DateTime, Utc};

    fn mock_sns_event(message: &str) -> SnsEvent {
        SnsEvent {
            records: vec![SnsRecord {
                event_version: "1.0".to_string(),
                event_subscription_arn: "arn:aws:sns:EXAMPLE".to_string(),
                event_source: "aws:sns".to_string(),
                sns: SnsMessage {
                    signature_version: "1".to_string(),
                    timestamp: DateTime::<Utc>::from_utc(
                        chrono::NaiveDateTime::from_timestamp(1_632_223_843, 0),
                        Utc,
                    ),
                    signature: "EXAMPLE".to_string(),
                    signing_cert_url: "EXAMPLE".to_string(),
                    message_id: "EXAMPLE".to_string(),
                    message: message.to_string(),
                    message_attributes: HashMap::new(),
                    sns_message_type: "Notification".to_string(),
                    unsubscribe_url: "EXAMPLE".to_string(),
                    topic_arn: "arn:aws:sns:EXAMPLE".to_string(),
                    subject: None,
                },
            }],
        }
    }

    #[tokio::test]
    async fn test_handler() {
        let message = r#"{"post":"This is a test post"}"#;
        let event = mock_sns_event(message);
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