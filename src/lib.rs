mod service;
mod storage;
// private
mod file_helpers;

use service::user::User;
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
pub async fn initial_user(user_id: String) -> Result<(), String> {
    let loaded_user = User::load(user_id.clone());
    // if loaded_user isNone, then create a new user
    // and save it to the file system
    if loaded_user.is_err() {
        let mut user = User::new(user_id.clone());
        user.enable_auto_save();
        let _ = user.register().await?;
        user.save();
        return Ok(());
    } else {
        return Ok(());
    }
}

#[wasm_bindgen]
pub async fn register_user(user_id: String) -> Result<String, String> {
    let user = User::load(user_id.clone())?;
    return user.register().await;
}

#[wasm_bindgen]
pub fn get_file_path_readable(user_id: String) -> String {
    return User::get_file_path_readable(user_id);
}

#[wasm_bindgen]
pub fn create_group(user_id: String, group_id: String) -> Result<String, String> {
    let mut user = User::load(user_id.clone())?;
    return user.create_group(group_id.clone());
}

#[wasm_bindgen]
pub async fn sync_mls_state(user_id: String) -> Result<(), String> {
    let mut user = User::load(user_id.clone())?;
    let _ = user.update().await?;
    Ok(())
}

#[wasm_bindgen]
pub async fn can_add_member_to_group(
    user_id: String,
    target_user_id: String,
) -> Result<bool, String> {
    let user = User::load(user_id.clone())?;
    let can_invite = user.can_invite(target_user_id).await;
    return Ok(can_invite);
}

#[wasm_bindgen]
pub async fn add_member_to_group(
    user_id: String,
    member_user_id: String,
    group_id: String,
) -> Result<(), String> {
    let mut user = User::load(user_id.clone())?;
    return user.add_member_to_group(member_user_id, group_id).await;
}

#[wasm_bindgen]
pub fn mls_encrypt_msg(user_id: String, msg: String, group_id: String) -> Result<String, String> {
    let mut user = User::load(user_id.clone())?;
    return user.send_msg(&msg, group_id);
}

#[wasm_bindgen]
pub fn mls_decrypt_msg(
    user_id: String,
    msg: String,
    sender_user_id: String,
    group_id: String,
) -> Result<String, String> {
    let user = User::load(user_id.clone())?;
    return user.read_msg(msg, sender_user_id, group_id);
}

#[wasm_bindgen]
pub async fn leave_group(user_id: String, group_id: String) -> Result<(), String> {
    let mut user = User::load(user_id.clone())?;
    return user.leave_group(group_id).await;
}
