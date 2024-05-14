use serde::Deserialize;
use serde::Serialize;
use lambda_http::{service_fn, Response, Body, Error, Request};
use serde_json::json;
use std::env;



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

async fn handler(request: Request) -> Result<Response<String>, Error> {
    let body = request.body();
    println!("Body: {:?}", body);
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

}