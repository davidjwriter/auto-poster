use serde::Deserialize;
use serde::Serialize;
use lambda_http::{service_fn, Response, Body, Error, Request};
use serde_json::json;
use std::env;



#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(handler);
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn get_twitter_api_key() -> Option<String> {
    env::var("TWITTER_API_KEY").ok()
}

async fn get_twitter_secret() -> Option<String> {
    env::var("TWITTER_SECRET").ok()
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