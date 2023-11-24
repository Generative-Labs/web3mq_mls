mod service;
mod storage;
// private
mod index_db_helper;

use openmls::framing::MlsMessageIn;
use service::{networking::NetworkingConfig, user::User};
use tls_codec::Deserialize;
use wasm_bindgen::prelude::*;

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
    if loaded_user.is_err() {
        let mut user = User::new(user_id.clone());
        user.enable_auto_save();
        let _ = user.register().await?;
        user.save().await;
        return Ok(());
    } else {
        return Ok(());
    }
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
pub async fn sync_mls_state(user_id: String) -> Result<(), String> {
    let mut user = User::load(user_id.clone()).await?;
    let _ = user.update().await?;
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
    return user.handle_mls_group_event(msg);
}

#[cfg(test)]
mod tests {
    use crate::service::user::User;

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
}
