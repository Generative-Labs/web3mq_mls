use openmls::prelude::*;
use sha2::{Digest, Sha256};

use tls_codec::{Deserialize, TlsVecU16, TlsVecU32};
use url::Url;

use crate::service::client_info::{ClientInfo, ClientKeyPackages, GroupMessage};
use crate::service::user::User;

use super::client_info::RegisterClientParams;
use super::networking::{ed25519_sign, get, post, NetworkingConfig};
pub struct Backend {
    ds_url: Url,
}

impl Backend {
    /// Register a new client with the server.
    pub async fn register_client(&self, user: &User) -> Result<String, String> {
        let mut url = self.ds_url.clone();
        url.set_path("/api/user/key_package/");

        let key_packages = user.key_packages();

        // convert user.key_packages() to TlsVecU32<(Vec<u8>, Vec<u8>)
        let mut vec = Vec::new();
        for (key, value) in key_packages {
            vec.push((key, value.tls_serialize_detached().unwrap()));
        }
        let tls_vec = TlsVecU32::from(vec);

        // let key_packages = user
        //     .key_packages()
        //     .into_iter()
        //     .map(|(b, kp)| {
        //         (
        //             b
        //             base64::encode(
        //                 kp.tls_serialize_detached()
        //                     .expect("Error serializing key package"),
        //             ),
        //         )
        //     })let key_packages = user.key_packages();
        //     .collect::<TlsVecU32<(Vec<u8>, Vec<u8>)>>();

        let now = instant::SystemTime::now();
        let timestamp = now
            .duration_since(instant::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let payload_hash = Backend::get_payload_hash(&user.user_id, tls_vec.clone(), timestamp);

        let private_key = NetworkingConfig::instance().get_private_key();

        // ed25519 encrypt
        let signature = ed25519_sign(&private_key, &payload_hash)
            .await
            .expect("Error signing");

        let client_info = RegisterClientParams {
            userid: user.user_id.clone(),
            timestamp,
            key_packages: tls_vec,
            payload_hash: payload_hash,
            web3mq_user_mainkey_signature: signature,
        };

        let response = post(&url, &client_info).await?;
        Ok(String::from_utf8(response).unwrap())
    }

    /// sha256_hash(
    ///    userid + base64_encode(json_dumps(key_package)) + timestamp
    /// )
    fn get_payload_hash(
        user_id: &str,
        key_package: TlsVecU32<(Vec<u8>, Vec<u8>)>,
        timestamp: u64,
    ) -> String {
        let content = format!(
            "{}{}{}",
            user_id,
            base64::encode(key_package.tls_serialize_detached().unwrap()),
            timestamp
        );
        // sha256 hash
        return Sha256::digest(content.as_bytes())
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
    }

    /// Get a list of all clients with name, ID, and key packages from the
    /// server.
    pub async fn list_clients(&self) -> Result<Vec<ClientInfo>, String> {
        let mut url = self.ds_url.clone();
        url.set_path("/clients/list");

        let response = get(&url).await?;
        match TlsVecU32::<ClientInfo>::tls_deserialize(&mut response.as_slice()) {
            Ok(clients) => Ok(clients.into()),
            Err(e) => Err(format!("Error decoding server response: {e:?}")),
        }
    }

    /// Get and reserve a key package for a client.
    pub async fn consume_key_package(&self, client_id: &[u8]) -> Result<KeyPackageIn, String> {
        let mut url = self.ds_url.clone();
        let path = "/clients/key_package/".to_string()
            + &base64::encode_config(client_id, base64::URL_SAFE);
        url.set_path(&path);

        let response = get(&url).await?;
        match KeyPackageIn::tls_deserialize(&mut response.as_slice()) {
            Ok(kp) => Ok(kp),
            Err(e) => Err(format!("Error decoding server response: {e:?}")),
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
        url.set_path("/send/welcome");

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
        let path = "/recv/".to_string()
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
