use openmls::prelude::*;
use serde_json::from_slice;
use sha2::{Digest, Sha256};

use tls_codec::Deserialize;
use url::Url;

use crate::service::client_info::{ClientKeyPackages, GroupMessage, QueryKeyPackagesParam};
use crate::service::user::User;

use super::client_info::{
    KeyPackagesResult, RegisterKeyPackageParams, Response, SendMessageParams, StatesRequestParams,
    StatesRequestResult,
};
use super::networking::{ed25519_sign, get, post, NetworkingConfig, _post};

trait RequestSigner {
    fn sign_request(
        user_id: &str,
        body: &str,
        private_key: &str,
        timestamp: Option<u128>,
    ) -> (String, String, u128);
    fn get_payload_hash(user_id: &str, body: &str, timestamp: u128) -> String;
}

pub struct Backend {
    ds_url: Url,
}

impl RequestSigner for Backend {
    fn sign_request(
        user_id: &str,
        body: &str,
        private_key: &str,
        timestamp: Option<u128>,
    ) -> (String, String, u128) {
        // if timestamp is None, then use current timestamp
        let timestamp = timestamp.unwrap_or_else(|| {
            let now = instant::SystemTime::now();
            now.duration_since(instant::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        });
        let payload_hash = Backend::get_payload_hash(user_id, body, timestamp);
        // ed25519 encrypt
        let signature = ed25519_sign(&private_key, &payload_hash).expect("Error signing");
        return (signature, payload_hash, timestamp);
    }

    fn get_payload_hash(user_id: &str, body: &str, timestamp: u128) -> String {
        let content = format!("{}{}{}", user_id, body, timestamp);
        // sha256 hash
        return Sha256::digest(content.as_bytes())
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
    }
}

impl Backend {
    /// Register a new key package.
    pub async fn register_key_packages(&self, user: &User) -> Result<String, String> {
        let mut url = self.ds_url.clone();
        url.set_path("/api/user/key_package/");
        let key_packages = user.key_packages_map();

        let private_key = NetworkingConfig::instance().get_private_key();
        let json_string = serde_json::to_string(&key_packages.clone()).expect("Error serializing");
        let body = json_string;

        let (signature, payload_hash, timestamp) =
            Backend::sign_request(&user.user_id, &body, &private_key, None);

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

    /// Get and reserve a key package for a client.
    pub async fn consume_key_package(&self, user_id: &str) -> Result<KeyPackageIn, String> {
        let mut url = self.ds_url.clone();
        url.set_path("/api/user/get_key_package/");

        let msg_params = QueryKeyPackagesParam {
            target_userid: user_id.to_string(),
            timestamp: instant::SystemTime::now()
                .duration_since(instant::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        };

        let response = _post(&url, &msg_params).await?;
        let response: Response<KeyPackagesResult> = from_slice(&response)
            .map_err(|e| format!("Error decoding server response: {:?}", e))?;

        let first_key_package_base64 = response
            .data
            .key_packages
            .values()
            .last()
            .ok_or("No key packages found".to_string())?;

        print!(
            "debug:first_key_package_base64: {:?}",
            first_key_package_base64
        );

        let first_key_package_bytes =
            base64::decode_config(first_key_package_base64, base64::URL_SAFE)
                .map_err(|_| "Failed to decode base64 string".to_string())?;

        KeyPackageIn::tls_deserialize(&mut first_key_package_bytes.as_slice())
            .map_err(|e| format!("Error decoding server response: {:?}", e))
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
    pub async fn send_welcome(
        &self,
        welcome_msg: &MlsMessageOut,
        sender: &str,
        receiver: &str,
        group_id: &str,
    ) -> Result<(), String> {
        let mut url = self.ds_url.clone();
        url.set_path("/api/group/mls_state/");

        let msg_base64_string = base64::encode(welcome_msg.tls_serialize_detached().unwrap());
        let body = group_id.to_owned() + receiver + &msg_base64_string;

        let private_key = NetworkingConfig::instance().get_private_key();

        let (signature, payload_hash, timestamp) =
            Backend::sign_request(sender, &body, &private_key, None);

        let msg_params = SendMessageParams {
            userid: sender.to_string(),
            timestamp: timestamp,
            web3mq_user_signature: signature,
            payload_hash: payload_hash,
            mls_msg: msg_base64_string,
            groupid: group_id.to_string(),
            recipients_topicid: receiver.to_string(),
        };

        // The response should be empty.
        let _response = _post(&url, &msg_params).await?;
        Ok(())
    }

    /// Send a group message.
    pub async fn send_msg(
        &self,
        group_msg: &GroupMessage,
        sender: &str,
        group_id: &str,
    ) -> Result<(), String> {
        let mut url: Url = self.ds_url.clone();
        url.set_path("/api/group/mls_state/");

        let receiver_user_id = &group_msg.recipient;
        let msg_base64_string = base64::encode(group_msg.msg.tls_serialize_detached().unwrap());
        let body = group_id.to_owned() + receiver_user_id + &msg_base64_string;
        let private_key = NetworkingConfig::instance().get_private_key();

        let (signature, payload_hash, timestamp) =
            Backend::sign_request(sender, &body, &private_key, None);

        let msg_params = SendMessageParams {
            userid: sender.to_string(),
            timestamp: timestamp,
            web3mq_user_signature: signature,
            payload_hash: payload_hash,
            mls_msg: msg_base64_string,
            groupid: group_id.to_string(),
            recipients_topicid: receiver_user_id.clone(),
        };

        // The response should be empty.
        let _response = _post(&url, &msg_params).await?;
        Ok(())
    }

    /// Get a list of all new messages for the user.
    pub async fn recv_msgs(
        &self,
        user: &User,
        groups: Vec<String>,
    ) -> Result<Vec<MlsMessageIn>, String> {
        let mut url = self.ds_url.clone();
        let path = "/api/group/get_mls_state/".to_string();
        url.set_path(&path);

        let private_key = NetworkingConfig::instance().get_private_key();
        // json_dumps groups, and then base64 encode
        let groups_json = serde_json::to_string(&groups).unwrap();
        let body = groups_json;
        let (signature, payload_hash, timestamp) =
            Backend::sign_request(&user.user_id, &body, &private_key, None);

        // let sync_time = *user.mls_sync_timestamp.borrow();
        let msg_params = StatesRequestParams {
            userid: user.user_id.to_string(),
            timestamp: timestamp,
            web3mq_user_signature: signature,
            payload_hash: payload_hash,
            groupid_list: groups,
            timestamp_gte: 0, // TODO: for debug
        };

        let response = _post(&url, &msg_params).await?;
        let response: Response<StatesRequestResult> = from_slice(&response)
            .map_err(|e| format!("Error decoding server response: {:?}", e))?;

        // get the MlsMessageIn from every response.data.mls_states.value
        // flat all response.data.mls_states.values
        let mut results: Vec<MlsMessageIn> = Vec::with_capacity(response.data.mls_states.len());

        for v in response.data.mls_states.values() {
            for value in v {
                let response = base64::decode(value).unwrap();
                let msg = MlsMessageIn::tls_deserialize(&mut response.as_slice()).unwrap();
                results.push(msg);
            }
        }

        // replace now
        user.mls_sync_timestamp.replace(
            instant::SystemTime::now()
                .duration_since(instant::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        );

        Ok(results)
    }

    pub fn reset_ds_url(&mut self, ds_url: &str) {
        self.ds_url = Url::parse(ds_url).unwrap();
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            ds_url: Url::parse(&NetworkingConfig::instance().get_base_url()).unwrap(),
        }
    }
}
