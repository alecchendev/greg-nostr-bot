use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use hyper::body::HttpBody;
use hyper::client::HttpConnector;
use hyper::{Body, Client, Method, Request};
use hyper_tls::HttpsConnector;

// use futures::StreamExt;
use futures_util::sink::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, Connector, MaybeTlsStream, WebSocketStream,
};

// use nostr_rust;
use nostr_types::{Event, EventKind, PreEvent, PrivateKey, PublicKey, Unixtime};

use tokio::sync::{Mutex, MutexGuard};

use std::sync::Arc;

use hyper::Uri;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, world!");

    let relays = read_relays()?;

    let env = read_env()?;
    let _api_key = env.get("API_KEY").unwrap();
    let bearer_token = env.get("BEARER_TOKEN").unwrap();
    let private_key = PrivateKey::try_from_bech32_string(env.get("PRIVATE_KEY").unwrap())?;

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    let mut nostr_client = NostrClient::new().await;
    nostr_client.add_relays(&relays).await;

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
        "post" => post_note(&mut nostr_client, "Hello, nostr!", &private_key).await?,
        // "get_notes" => get_notes(),
        _ => println!("No command found"),
    };

    Ok(())
}

async fn post_note(
    nostr_client: &mut NostrClient,
    text: &str,
    private_key: &nostr_types::PrivateKey,
) -> Result<(), Box<dyn Error>> {
    let pre_event = PreEvent {
        /// The public key of the actor who is creating the event
        pubkey: private_key.public_key(),
        /// The time at which the event was created
        created_at: Unixtime(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        ),
        /// The kind of event
        kind: EventKind::TextNote,
        /// A set of tags that apply to the event
        tags: Vec::new(),
        /// The content of the event
        content: text.to_string(),
        /// An optional verified time for the event (using OpenTimestamp)
        ots: None,
    };
    let event = Event::new(pre_event, private_key)?;
    assert!(event.verify(None).is_ok());
    nostr_client.publish_event(&event).await?;
    Ok(())
}

struct NostrClient {
    relays: HashMap<String, Arc<Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
}

impl NostrClient {
    async fn new() -> Self {
        Self {
            relays: HashMap::new(),
        }
    }

    async fn add_relays(&mut self, relay_urls: &Vec<String>) -> Result<(), Box<dyn Error>> {
        for url in relay_urls {
            self.add_relay(url).await?;
        }
        Ok(())
    }

    async fn add_relay(&mut self, url: &String) -> Result<(), Box<dyn Error>> {
        if self.relays.contains_key(url) {
            return Ok(());
        }
        let uri = url.parse::<Uri>().unwrap();
        let (ws_stream, response) = connect_async(uri).await?;
        println!("Relay: {} | connect response: {:?}", url, response);
        self.relays.insert(url.clone(), Arc::new(Mutex::new(ws_stream)));
        Ok(())
    }

    async fn publish_event(&mut self, event: &Event) -> Result<(), Box<dyn Error>> {
        let event_json = json!(["EVENT", event]).to_string();
        let message = Message::text(event_json.clone());
        for (url, ws_stream) in self.relays.iter() {
            let mut ws_stream = ws_stream.lock().await;

            ws_stream.send(message.clone()).await?;
            println!("Relay: {} | sent event: {:?}", url, event);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TweetData {
    edit_history_tweet_ids: Option<Vec<String>>,
    id: String,
    text: String,
}

async fn get_stream(
    client: &Client<HttpsConnector<HttpConnector>>,
    bearer_token: &str,
) -> Result<(), Box<dyn Error>> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("https://api.twitter.com/2/tweets/search/stream")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer ".to_owned() + bearer_token)
        .body(Body::empty())?;

    let mut res: hyper::Response<Body> = client.request(req).await?;

    println!("status: {}", res.status());

    while let Some(chunk) = res.body_mut().data().await {
        let chunk = chunk?;
        let s = String::from_utf8(chunk.to_vec())?;
        if s.trim() == "" {
            // println!("empty chunk");
            continue;
        }
        // spawn a new thread to handle the chunk
        tokio::spawn(async move {
            handle_tweet(s).unwrap();
        });
    }

    // let buf = hyper::body::to_bytes(res).await?;

    // println!("body: {:?}", buf);

    // let json: Value = serde_json::from_slice(&buf)?;

    Ok(())
}

fn handle_tweet(s: String) -> Result<(), Box<dyn Error>> {
    println!("chunk: {}", s);

    let json: Value = serde_json::from_str(&s)?;
    let data: TweetData = serde_json::from_value(json["data"].clone())?;

    println!("data: {:?}", data);

    // post data.text to nostr!

    Ok(())
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
