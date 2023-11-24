use ed25519_dalek::{Signer, SigningKey};
use reqwest::{header::HeaderMap, Client, StatusCode};
use serde_json::Value;
use url::Url;

use tls_codec::Serialize;

use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Mutex};

use base64;

pub struct NetworkingConfig {
    pub base_url: Mutex<String>,
    pub pubkey: Mutex<String>,
    pub did_key: Mutex<String>,
    pub private_key: Mutex<String>,
}

lazy_static! {
    static ref SINGLETON_CONFIG: NetworkingConfig = NetworkingConfig {
        base_url: Mutex::new(String::new()),
        pubkey: Mutex::new(String::new()),
        did_key: Mutex::new(String::new()),
        private_key: Mutex::new(String::new())
    };
}

impl NetworkingConfig {
    pub fn instance() -> &'static NetworkingConfig {
        &SINGLETON_CONFIG
    }
}

impl NetworkingConfig {
    pub fn set_base_url(&self, value: String) {
        *self.base_url.lock().unwrap() = value;
    }

    pub fn set_did_key(&self, value: String) {
        *self.did_key.lock().unwrap() = value;
    }

    pub fn set_pubkey(&self, value: String) {
        *self.pubkey.lock().unwrap() = value;
    }

    pub fn set_private_key(&self, value: String) {
        *self.private_key.lock().unwrap() = value;
    }

    pub fn get_private_key(&self) -> String {
        self.private_key.lock().unwrap().to_string()
    }

    pub fn setup(
        &self,
        base_url: Option<String>,
        pubkey: Option<String>,
        did_key: Option<String>,
        private_key: Option<String>,
    ) {
        if let Some(base_url) = base_url {
            self.set_base_url(base_url);
        }
        if let Some(pubkey) = pubkey {
            self.set_pubkey(pubkey);
        }
        if let Some(did_key) = did_key {
            self.set_did_key(did_key);
        }
        if let Some(private_key) = private_key {
            self.set_private_key(private_key);
        }
    }

    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "web3mq-request-pubkey",
            self.pubkey.lock().unwrap().parse().unwrap(),
        );
        headers.insert("didkey", self.did_key.lock().unwrap().parse().unwrap());
        headers
    }
}

pub async fn ed25519_sign(private_key: &str, sign_content: &str) -> Result<String, String> {
    // private_key is hex encoded, should convert it to bytes
    let private_key_bytes = hex::decode(private_key).map_err(|_| "Failed to decode private key")?;
    let private_key_bytes: &[u8; 32] = private_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Invalid private key length")?;
    let key_pair = SigningKey::from_bytes(private_key_bytes);
    let signature = key_pair.sign(sign_content.as_bytes());
    Ok(hex::encode(signature.to_bytes()))
}

pub async fn post(url: &Url, msg: &impl Serialize) -> Result<Vec<u8>, String> {
    let serialized_msg = msg.tls_serialize_detached().unwrap();
    log::debug!("Post {:?}", url);
    log::trace!("Payload: {:?}", serialized_msg);

    let client = Client::builder()
        .default_headers(NetworkingConfig::instance().default_headers())
        .build()
        .unwrap();

    let response = client
        .post(url.to_string())
        .body(serialized_msg)
        .send()
        .await;

    if let Ok(r) = response {
        if r.status() != StatusCode::OK {
            return Err(format!("Error status code {:?}", r.status()));
        }
        match r.bytes().await {
            Ok(bytes) => Ok(bytes.as_ref().to_vec()),
            Err(e) => Err(format!("Error retrieving bytes from response: {e:?}")),
        }
    } else {
        Err(format!("ERROR: {:?}", response.err()))
    }
}

pub async fn get(url: &Url) -> Result<Vec<u8>, String> {
    let client = Client::builder()
        .default_headers(NetworkingConfig::instance().default_headers())
        .build()
        .unwrap();

    log::debug!("Get {:?}", url);
    let response = client.get(url.to_string()).send().await;
    if let Ok(r) = response {
        if r.status() != StatusCode::OK {
            return Err(format!("Error status code {:?}", r.status()));
        }
        match r.bytes().await {
            Ok(bytes) => Ok(bytes.as_ref().to_vec()),
            Err(e) => Err(format!("Error retrieving bytes from response: {e:?}")),
        }
    } else {
        Err(format!("ERROR: {:?}", response.err()))
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct ClientInfo {
    pub user_id: String,
    pub key_packages: HashMap<String, String>,
}

impl ClientInfo {
    fn new(user_id: String, key_packages: HashMap<String, String>) -> Self {
        Self {
            user_id,
            key_packages,
        }
    }
}

///
pub async fn _post(url: &Url, msg: &impl serde::Serialize) -> Result<Vec<u8>, String> {
    let json_string = serde_json::to_string(msg).unwrap();
    let body: HashMap<String, Value> = serde_json::from_str(&json_string).unwrap();
    let client = Client::builder()
        .default_headers(NetworkingConfig::instance().default_headers())
        .build()
        .unwrap();
    let response = client.post(url.to_string()).json(&body).send().await;
    if let Ok(r) = response {
        if r.status() != StatusCode::OK {
            return Err(format!("Error status code {:?}", r.status()));
        }
        match r.bytes().await {
            Ok(bytes) => Ok(bytes.as_ref().to_vec()),
            Err(e) => Err(format!("Error retrieving bytes from response: {e:?}")),
        }
    } else {
        Err(format!("ERROR: {:?}", response.err()))
    }
}
