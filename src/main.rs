use std::backtrace::BacktraceStatus;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use hyper::client::HttpConnector;
use hyper::header::{HeaderValue, AUTHORIZATION};
use hyper::{Body, Client, Method, Request, Uri};
use hyper_tls::HttpsConnector;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, world!");

    let relays = read_relays()?;

    let env = read_env()?;
    let api_key = env.get("API_KEY").unwrap();
    let bearer_token = env.get("BEARER_TOKEN").unwrap();

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    // Cli
    let args: Vec<String> = env::args().collect();
    let empty_str = "".to_string();
    let arg = args.get(1).unwrap_or(&empty_str);

    match arg.as_str() {
        "get_stream" => get_stream(&client, bearer_token)
            .await
            .and_then(|_| Ok(()))?,
        "get_rules" => get_stream_rules(&client, bearer_token)
            .await
            .and_then(|_| Ok(()))?,
        // "create_stream_rule" => create_stream_rule(&client, bearer_token).await?,
        "create_greg" => {
            create_only_user_stream_rule(&client, bearer_token, "greg16676935420").await?
        }
        "create_alec" => create_only_user_stream_rule(&client, bearer_token, "alecchendev").await?,
        // "delete_stream_rule" => delete_stream_rule(&client, bearer_token, vec!["1611155413902180354".to_string()]).await?,
        "delete_greg" => {
            delete_only_user_stream_rule(&client, bearer_token, "greg16676935420").await?
        }
        "delete_alec" => delete_only_user_stream_rule(&client, bearer_token, "alecchendev").await?,
        _ => println!("No command found"),
    };

    Ok(())
}

async fn get_stream(
    client: &Client<HttpsConnector<HttpConnector>>,
    bearer_token: &str,
) -> Result<Value, Box<dyn Error>> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("https://api.twitter.com/2/tweets/search/stream")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer ".to_owned() + bearer_token)
        .body(Body::empty())?;

    let res: hyper::Response<Body> = client.request(req).await?;

    println!("status: {}", res.status());

    let buf = hyper::body::to_bytes(res).await?;

    println!("body: {:?}", buf);

    let json: Value = serde_json::from_slice(&buf)?;

    Ok(json)
}

async fn get_stream_rules(
    client: &Client<HttpsConnector<HttpConnector>>,
    bearer_token: &str,
) -> Result<Value, Box<dyn Error>> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("https://api.twitter.com/2/tweets/search/stream/rules")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer ".to_owned() + bearer_token)
        .body(Body::empty())?;

    let res = client.request(req).await?;

    println!("status: {}", res.status());

    let buf = hyper::body::to_bytes(res).await?;

    println!("body: {:?}", buf);

    let json: Value = serde_json::from_slice(&buf)?;

    Ok(json)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Rule {
    id: String,
    value: String,
    tag: String,
}

async fn delete_only_user_stream_rule(
    client: &Client<HttpsConnector<HttpConnector>>,
    bearer_token: &str,
    user: &str,
) -> Result<(), Box<dyn Error>> {
    let res = get_stream_rules(&client, bearer_token).await?;

    let data = res["data"]
        .as_array()
        .expect("stream rules data is not an array");

    let rules: Vec<Rule> = data
        .iter()
        .map(|rule| serde_json::from_value(rule.clone()).expect("could not deserialize rule"))
        .collect();

    let only_user_id: Option<String> = rules
        .iter()
        .find(|rule| rule.tag == format!("from {}", user))
        .map(|rule| rule.id.clone());

    delete_stream_rule(client, bearer_token, vec![only_user_id.unwrap()]).await?;

    Ok(())
}

async fn delete_stream_rule(
    client: &Client<HttpsConnector<HttpConnector>>,
    bearer_token: &str,
    ids: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let payload = json!({
        "delete": {
            "ids": ids
        }
    })
    .to_string();

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

async fn create_only_user_stream_rule(
    client: &Client<HttpsConnector<HttpConnector>>,
    bearer_token: &str,
    user: &str,
) -> Result<(), Box<dyn Error>> {
    let payload = json!({
        "add": [
            {
                "value": format!("from:{}", user),
                "tag": format!("from {}", user)
            }
        ]
    })
    .to_string();

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
