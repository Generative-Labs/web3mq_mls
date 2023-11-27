use std::collections::HashMap;

use openmls::prelude::*;
use serde_json::from_slice;
use sha2::{Digest, Sha256};

use tls_codec::{Deserialize, TlsVecU16};
use url::Url;

use crate::service::client_info::{ClientKeyPackages, GroupMessage};
use crate::service::user::User;

use super::client_info::{KeyPackagesResult, RegisterKeyPackageParams, Response};
use super::networking::{ed25519_sign, get, post, NetworkingConfig, _post};

trait RequestSigner {
    fn sign_request(user_id: &str, body: &str, private_key: &str) -> (String, String, u128);
}

pub struct Backend {
    ds_url: Url,
}

impl RequestSigner for Backend {
    ///
    fn sign_request(user_id: &str, body: &str, private_key: &str) -> (String, String, u128) {
        let now = instant::SystemTime::now();
        let timestamp = now
            .duration_since(instant::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let payload_hash = Backend::get_payload_hash(user_id, body, timestamp);

        // ed25519 encrypt
        let signature = ed25519_sign(&private_key, &payload_hash).expect("Error signing");
        return (signature, payload_hash, timestamp);
    }
}

impl Backend {
    /// Register a new key package.
    pub async fn register_client(&self, user: &User) -> Result<String, String> {
        let mut url = self.ds_url.clone();
        url.set_path("/api/user/key_package/");
        let key_packages = user.key_packages_map();

        let private_key = NetworkingConfig::instance().get_private_key();
        let json_string = serde_json::to_string(&key_packages.clone()).expect("Error serializing");
        let body = base64::encode(json_string);

        let (signature, payload_hash, timestamp) =
            Backend::sign_request(&user.user_id, &body, &private_key);

        let client_info = RegisterKeyPackageParams {
            userid: user.user_id.clone(),
            timestamp,
            key_packages: key_packages.clone(),
            payload_hash: payload_hash,
            web3mq_user_signature: signature,
        };

        let response = _post(&url, &client_info).await?;
        Ok(String::from_utf8(response).unwrap())
    }

    // fn get_payload_hash(
    //     user_id: &str,
    //     key_package: HashMap<String, String>,
    //     timestamp: u128,
    // ) -> String {
    //     // convert a hash map key_package to bytes
    //     let json_string = serde_json::to_string(&key_package).expect("Error serializing");
    //     let content = format!("{}{}{}", user_id, base64::encode(json_string), timestamp);
    //     // sha256 hash
    //     return Sha256::digest(content.as_bytes())
    //         .iter()
    //         .map(|b| format!("{:02x}", b))
    //         .collect::<String>();
    // }

    fn get_payload_hash(user_id: &str, body: &str, timestamp: u128) -> String {
        // convert a hash map key_package to bytes
        // let json_string = serde_json::to_string(&key_package).expect("Error serializing");
        let content = format!("{}{}{}", user_id, body, timestamp);
        // sha256 hash
        return Sha256::digest(content.as_bytes())
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
    }

    // /// Get a list of all clients with name, ID, and key packages from the
    // /// server.
    // pub async fn list_clients(&self) -> Result<Vec<ClientInfo>, String> {
    //     let mut url = self.ds_url.clone();
    //     url.set_path("/clients/list");

    //     let response = get(&url).await?;
    //     match TlsVecU32::<ClientInfo>::tls_deserialize(&mut response.as_slice()) {
    //         Ok(clients) => Ok(clients.into()),
    //         Err(e) => Err(format!("Error decoding server response: {e:?}")),
    //     }
    // }

    /// Get and reserve a key package for a client.
    pub async fn consume_key_package(&self, user_id: &str) -> Result<KeyPackageIn, String> {
        let mut url = self.ds_url.clone();
        let path = "/api/user/key_package/".to_string() + "&target_user_id=" + user_id;
        url.set_path(&path);

        let response = get(&url).await?;

        let response: Response<KeyPackagesResult> = match from_slice(&response) {
            Ok(r) => r,
            Err(e) => return Err(format!("Error decoding server response: {:?}", e)),
        };

        let first_key_package_base64 = match response.data.key_packages.values().last() {
            Some(kp) => kp,
            None => return Err("No key packages found".to_string()),
        };

        let first_key_package_bytes = match base64::decode(first_key_package_base64) {
            Ok(bytes) => bytes,
            Err(_) => return Err("Failed to decode base64 string".to_string()),
        };

        match KeyPackageIn::tls_deserialize(&mut first_key_package_bytes.as_slice()) {
            Ok(kp) => Ok(kp),
            Err(e) => return Err(format!("Error decoding server response: {:?}", e)),
        }
    }

    /// Publish client additional key packages.
    pub async fn publish_key_packages(
        &self,
        user: &User,
        ckp: &ClientKeyPackages,
    ) -> Result<(), String> {
        let mut url = self.ds_url.clone();
        let path = "/clients/key_packages/".to_string()
            + &base64::encode_config(user.identity.borrow().identity(), base64::URL_SAFE);
        url.set_path(&path);

        // The response should be empty.
        let _response = post(&url, &ckp).await?;
        Ok(())
    }

    /// Send a welcome message.
    pub async fn send_welcome(&self, welcome_msg: &MlsMessageOut) -> Result<(), String> {
        let mut url = self.ds_url.clone();
        url.set_path("/api/group/mls_state/");

        // The response should be empty.
        let _response = post(&url, welcome_msg).await?;
        Ok(())
    }

    /// Send a group message.
    pub async fn send_msg(&self, group_msg: &GroupMessage) -> Result<(), String> {
        let mut url = self.ds_url.clone();
        url.set_path("/send/message");

        // The response should be empty.
        let _response = post(&url, group_msg).await?;
        Ok(())
    }

    /// Get a list of all new messages for the user.
    pub async fn recv_msgs(&self, user: &User) -> Result<Vec<MlsMessageIn>, String> {
        let mut url = self.ds_url.clone();
        // TODO: Parameters are subject to change.
        let path = "/api/group/mls_state/".to_string()
            + &base64::encode_config(user.identity.borrow().identity(), base64::URL_SAFE);
        url.set_path(&path);

        let response = get(&url).await?;
        match TlsVecU16::<MlsMessageIn>::tls_deserialize(&mut response.as_slice()) {
            Ok(r) => Ok(r.into()),
            Err(e) => Err(format!("Invalid message list: {e:?}")),
        }
    }

    /// Reset the DS.
    pub async fn reset_server(&self) {
        let mut url = self.ds_url.clone();
        url.set_path("reset");
        get(&url).await.unwrap();
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            ds_url: Url::parse("http://localhost:8080").unwrap(),
        }
    }
}
