use ed25519_dalek::{Signer, SigningKey};
use reqwest::{header::HeaderMap, Client, StatusCode};
use url::Url;

use tls_codec::Serialize;

use lazy_static::lazy_static;
use std::sync::Mutex;

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
    let private_key_bytes =
        base64::decode(&private_key.as_bytes()).map_err(|_| "Failed to decode private key")?;
    let private_key_bytes: &[u8; 32] = private_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Invalid private key length")?;
    let keypair = SigningKey::from_bytes(private_key_bytes);
    let signature = keypair.sign(sign_content.as_bytes());
    Ok(base64::encode(signature.to_bytes()))
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
