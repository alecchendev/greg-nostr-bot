use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use hyper::{Client, Request, Body, Method, Uri};
use hyper::header::{HeaderValue, AUTHORIZATION};

use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, world!");

    let relays = read_relays()?;

    let env = read_env()?;
    let api_key = env.get("API_KEY").unwrap();
    let bearer_token = env.get("BEARER_TOKEN").unwrap();

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    create_only_greg_stream_rule(&client, bearer_token).await?;

    Ok(())
}

async fn create_only_greg_stream_rule(client: &Client<HttpsConnector<HttpConnector>>, bearer_token: &str) -> Result<(), Box<dyn Error>> {
    let payload = json!({
        "add": [
            {
                "value": "from:greg16676935420",
                "tag": "from greg"
            }
        ] 
    }).to_string();
    
    let req = Request::builder()
        .method(Method::POST)
        .uri("https://api.twitter.com/2/tweets/search/stream/rules")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer ".to_owned() + bearer_token)
        .body(Body::from(payload.to_string()))?;

    let res = client.request(req).await?;

    println!("status: {}", res.status());

    let buf = hyper::body::to_bytes(res).await?;

    println!("body: {:?}", buf);

    Ok(())
}

fn read_env() -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut env = HashMap::new();
    let mut file = File::open(".env")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    for line in contents.lines() {
        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap().to_string();
        let value = parts.next().unwrap().to_string();
        env.insert(key, value);
    }
    Ok(env)
}

fn read_relays() -> Result<Vec<String>, Box<dyn Error>> {
    let mut relays = Vec::new();
    let mut file = File::open("relays.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    for line in contents.lines() {
        relays.push(line.to_string());
    }
    Ok(relays)
}