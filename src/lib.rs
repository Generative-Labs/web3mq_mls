use openmls::framing::MlsMessageIn;
use tls_codec::Deserialize;
use wasm_bindgen::prelude::*;

use service::{networking::NetworkingConfig, user::User};

mod service;
mod storage;
// private
mod index_db_helper;

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    alert(&format!("Hello, {}!", name));
}

#[wasm_bindgen]
pub fn setup_networking_config(
    base_url: Option<String>,
    pubkey: Option<String>,
    did_key: Option<String>,
    private_key: Option<String>,
) {
    NetworkingConfig::instance().setup(base_url, pubkey, did_key, private_key);
}

#[wasm_bindgen]
pub async fn initial_user(user_id: String) -> Result<(), String> {
    let loaded_user = User::load(user_id.clone()).await;
    // if loaded_user isNone, then create a new user
    // and save it to the file system
    return if loaded_user.is_err() {
        let mut user = User::new(user_id.clone());
        user.enable_auto_save();
        let _ = user.register().await?;
        user.save().await;
        Ok(())
    } else {
        Ok(())
    };
}

#[wasm_bindgen]
pub async fn register_user(user_id: String) -> Result<String, String> {
    let user = User::load(user_id.clone()).await?;
    return user.register().await;
}

#[wasm_bindgen]
pub async fn create_group(user_id: String, group_id: String) -> Result<String, String> {
    let mut user = User::load(user_id.clone()).await?;
    return user.create_group(group_id.clone()).await;
}

#[wasm_bindgen]
pub async fn sync_mls_state(user_id: String, group_ids: Vec<String>) -> Result<(), String> {
    let mut user = User::load(user_id.clone()).await?;
    let _ = user.update(group_ids).await?;
    Ok(())
}

#[wasm_bindgen]
pub async fn can_add_member_to_group(
    user_id: String,
    target_user_id: String,
) -> Result<bool, String> {
    let user = User::load(user_id.clone()).await?;
    let can_invite = user.can_invite(&target_user_id).await;
    return Ok(can_invite);
}

#[wasm_bindgen]
pub async fn add_member_to_group(
    user_id: String,
    member_user_id: String,
    group_id: String,
) -> Result<(), String> {
    let mut user = User::load(user_id.clone()).await?;
    return user.add_member_to_group(&member_user_id, group_id).await;
}

#[wasm_bindgen]
pub async fn mls_encrypt_msg(
    user_id: String,
    msg: String,
    group_id: String,
) -> Result<String, String> {
    let mut user = User::load(user_id.clone()).await?;
    return user.send_msg(&msg, group_id).await;
}

#[wasm_bindgen]
pub async fn mls_decrypt_msg(
    user_id: String,
    msg: String,
    sender_user_id: String,
    group_id: String,
) -> Result<String, String> {
    let user = User::load(user_id.clone()).await?;
    return user.read_msg(msg, sender_user_id, group_id);
}

#[wasm_bindgen]
pub async fn handle_mls_group_event(user_id: String, msg_bytes: Vec<u8>) -> Result<(), String> {
    let mut user = User::load(user_id.clone()).await?;
    let msg =
        MlsMessageIn::tls_deserialize(&mut msg_bytes.as_slice()).map_err(|e| e.to_string())?;
    return user.handle_mls_group_event(msg).await;
}

#[cfg(test)]
mod tests {

    use httpmock::{
        Method::{GET, POST},
        MockServer,
    };
    use serde_json::json;

    use crate::{
        initial_user,
        service::{
            backend::{self, Backend},
            networking::ed25519_sign,
            user::User,
        },
        setup_networking_config,
    };

    #[tokio::test]
    async fn test_persistent() {
        let user_id = "Alice".to_string();
        let loaded_user = User::load(user_id.clone()).await;
        // if loaded_user isNone, then create a new user
        // and save it to the file system
        if loaded_user.is_err() {
            let mut user = User::new(user_id.clone());
            user.enable_auto_save();
            user.save().await;
            print!("user created")
        } else {
            print!("user already exists")
        }
    }

    #[tokio::test]
    async fn test_ed25519() {
        let private_key = "212E8F31AD54D79E075A04802C2B307E339B3373072F80E721880702C052B637";
        let sign_content = "hello";
        let result_should_be = "E1B7A23CE8D2B8A81EAB627F5936606A853D215A9CA5E56BD3D9871692E731C7D32E40ABF8A4C0D547749E5BD2DAE6AFD7A200A11CDA79A0EEE35029F2E24E03".to_lowercase();
        let signature = ed25519_sign(private_key, sign_content);
        print!("signature: {:?}", signature);
        assert_eq!(signature.is_ok(), true);
        assert_eq!(signature.unwrap(), result_should_be);
    }

    #[tokio::test]
    async fn test_register_key_package() {
        // Start a lightweight mock server.
        let server = MockServer::start();

        // Create a mock on the server.
        let hello_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/user/key_package/")
                .query_param("word", "hello");
            then.status(200)
                .header("content-type", "text/html; charset=UTF-8")
                .body("Привет");
        });

        let user = User::new("Alice".to_string());

        let backend = Backend::default();
        backend.register_key_packages(&user).await;

        // user.register().await;

        hello_mock.assert();
    }

    #[tokio::test]
    async fn test_consume_key_package() {
        let user_id = "Alice".to_string();
        let mut user = User::new(user_id.clone());

        let private_key = "212E8F31AD54D79E075A04802C2B307E339B3373072F80E721880702C052B637";
        setup_networking_config(
            None,
            None,
            None,
            std::option::Option::Some(private_key.to_string()),
        );

        let server = MockServer::start();
        user.reset_ds_url(&server.base_url());

        // let path = format!("/api/user/key_packages/?target_user_id={}", user_id);
        // Create a mock on the server.
        let get_mock = server.mock(|when, then| {
            when.method(GET)
            .path("/api/user/key_packages/")
            .query_param("target_user_id", user_id.clone());
            then.status(200).json_body(json!({ "code": 0,  "msg": "ok",  "data": {"userid": user.user_id.clone(), "timestamp": 0, "web3mq_user_signature": "", "key_packages": user.key_packages_map()}  }));
        });

        let can_invite = user.can_invite(&user_id.clone()).await;
        assert_eq!(can_invite, true);
        get_mock.assert();
    }

    #[tokio::test]
    async fn test_post_key_package() {
        let user_id = "Alice".to_string();
        let mut user = User::new(user_id.clone());

        let private_key = "212E8F31AD54D79E075A04802C2B307E339B3373072F80E721880702C052B637";
        setup_networking_config(
            None,
            None,
            None,
            std::option::Option::Some(private_key.to_string()),
        );

        // Start a lightweight mock server.
        let server = MockServer::start();
        print!("server: {:?}", server.base_url());
        user.reset_ds_url(&server.base_url());

        let post_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/user/key_package/");
            then.status(200).json_body(json!({ "code": 0 , "msg": "ok",  "data": {"userid": user.user_id.clone(), "timestamp": 0, "web3mq_user_signature": "", "key_packages": user.key_packages_map()} }));
        });

        let result: Result<String, String> = user.register().await;
        print!("result: {:?}", result);
        assert_eq!(result.is_ok(), true);
        post_mock.assert();
    }
}
