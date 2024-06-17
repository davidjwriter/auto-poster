use deso_sdk;
use deso_sdk::Node;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::env;
use aws_lambda_events::event::sqs::SqsEvent;
use dotenv::dotenv;

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub uuid: String,
    pub post: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(handler);
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn get_deso_user() -> Option<String> {  
    env::var("DESO_USER").ok()
}

async fn get_deso_private_key() -> Option<String> {
    env::var("DESO_PRIVATE_KEY").ok()
}

async fn worker(body: &str) -> Result<String, Error> {
    dotenv().ok();
    println!("Raw body: {}", body);
    let post: Post = serde_json::from_str(&body).expect("Couldn't parse json post");

    let body = post.post;
    println!("Body: {:?}", body);
    let comment = String::from("If you like this kind of content, make sure to checkout my newsletter and remember, run with joy! https://davidjmeyer.substack.com");
    println!("{}", body);
    let deso_account = deso_sdk::DesoAccountBuilder::new()
        .public_key(get_deso_user().await.unwrap())
        .seed_hex_key(get_deso_private_key().await.unwrap())
        .node(Node::MAIN)
        .build()
        .unwrap();

    let post_data = deso_sdk::SubmitPostDataBuilder::new()
        .body(body)
        .public_key(get_deso_user().await.unwrap())
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
        .public_key(get_deso_user().await.unwrap())
        .build()
        .unwrap();

    deso_sdk::create_post(&deso_account, &comment_data).await?;

    Ok(String::from("Success!"))
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
        let message = r#"{"post":"This is a test post", "uuid": "hello"}"#;
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
