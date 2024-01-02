use std::borrow::BorrowMut;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::str::FromStr;
use std::{cell::RefCell, collections::HashMap, str};

use async_trait::async_trait;
use openmls::messages::group_info::VerifiableGroupInfo;
use openmls::prelude::*;
use openmls::test_utils::{bytes_to_hex, hex_to_bytes};
use openmls_traits::OpenMlsProvider;

use crate::file_helpers;
use crate::service::backend::Backend;
use crate::service::client_info::GroupMessage;
use crate::service::conversation::{Conversation, ConversationMessage};
use crate::service::identity::Identity;
use crate::storage::persistent_crypto::OpenMlsRustPersistentCrypto;

const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Contact {
    username: String,
    id: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Group {
    group_id: String,
    pub(crate) conversation: Conversation,
    mls_group: RefCell<MlsGroup>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct User {
    pub(crate) user_id: String,
    pub(crate) groups: RefCell<HashMap<String, Group>>,
    group_list: HashSet<String>,
    pub(crate) identity: RefCell<Identity>,
    #[serde(skip)]
    backend: Backend,
    #[serde(skip)]
    crypto: OpenMlsRustPersistentCrypto,
    autosave_enabled: bool,
    /// Timestamps of the last mls sync.
    pub(crate) mls_sync_timestamp: RefCell<u128>,
}

#[derive(PartialEq)]
pub enum PostUpdateActions {
    None,
    Remove,
}

#[async_trait]
pub trait UserStore {
    async fn load(user_id: &str) -> Result<User, String>;
    async fn save(&mut self);
}

#[cfg(not(target_family = "wasm"))]
impl User {
    fn get_file_path(user_name: &String) -> PathBuf {
        file_helpers::get_file_path(&("web3mq_".to_owned() + user_name + ".json"))
    }

    fn load_from_file(input_file: &File) -> Result<Self, String> {
        // Prepare file reader.
        let reader = BufReader::new(input_file);

        // Read the JSON contents of the file as an instance of `User`.
        match serde_json::from_reader::<BufReader<&File>, User>(reader) {
            Ok(user) => Ok(user),
            Err(e) => Result::Err(e.to_string()),
        }
    }

    fn save_to_file(&self, output_file: &File) {
        let writer = BufWriter::new(output_file);
        match serde_json::to_writer_pretty(writer, &self) {
            Ok(()) => log::info!("User serialized"),
            Err(e) => log::error!("Error serializing user: {:?}", e.to_string()),
        }
    }
}

#[async_trait]
#[cfg(target_family = "wasm")]
impl User {
    async fn load_from_file(database: &Rexie, user_id: &str) -> Result<User, String> {
        let transaction = database
            .transaction(
                &[DatabaseType::User.store_name()],
                TransactionMode::ReadOnly,
            )
            .map_err(|e| e.to_string())?;

        let store = transaction
            .store(DatabaseType::User.store_name().as_str())
            .map_err(|e| e.to_string())?;

        let key = serde_wasm_bindgen::to_value(&user_id).unwrap();
        let user_value = store.get(&key).await.map_err(|e| e.to_string())?;
        let serializable_user: Option<User> = serde_wasm_bindgen::from_value(user_value).unwrap();
        match serializable_user {
            Some(user) => Ok(user),
            None => Err("Error load user".to_string()),
        }
    }

    async fn save_to_file(&self) {
        let database = index_db_helper::build_database(&self.user_id)
            .await
            .expect("Error building database");

        let transaction = database
            .transaction(
                &[DatabaseType::User.store_name()],
                TransactionMode::ReadWrite,
            )
            .expect("Error creating transaction");

        let store = transaction
            .store(DatabaseType::User.store_name().as_str())
            .expect("Error getting store");

        let ks: wasm_bindgen::prelude::JsValue = serde_wasm_bindgen::to_value(&self).unwrap();
        let key = serde_wasm_bindgen::to_value(&self.user_id.clone()).unwrap();
        store
            .put(&ks, Some(&key))
            .await
            .expect("Error putting value");

        transaction
            .done()
            .await
            .expect("Error committing transaction");
    }
}

#[async_trait]
#[cfg(not(target_family = "wasm"))]
impl UserStore for User {
    async fn load(user_id: &str) -> Result<User, String> {
        let input_path = User::get_file_path(&user_id.to_string());
        match File::open(input_path) {
            Err(e) => {
                log::error!("Error loading user state: {:?}", e.to_string());
                Err(e.to_string())
            }
            Ok(input_file) => {
                let user_result = User::load_from_file(&input_file);
                if user_result.is_ok() {
                    let mut user = user_result.ok().unwrap();
                    match user.crypto.load_keystore(user_id.to_string()) {
                        Ok(_) => {
                            let groups = user.groups.get_mut();
                            for group_name in &user.group_list {
                                let mlsgroup = MlsGroup::load(
                                    &GroupId::from_slice(group_name.as_bytes()),
                                    user.crypto.key_store(),
                                );
                                let grp = Group {
                                    mls_group: RefCell::new(mlsgroup.unwrap()),
                                    group_id: group_name.clone(),
                                    conversation: Conversation::default(),
                                };
                                groups.insert(group_name.clone(), grp);
                            }
                            Ok(user)
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    user_result
                }
            }
        }
    }

    async fn save(&mut self) {
        let output_path = User::get_file_path(&self.user_id);
        match File::create(output_path) {
            Err(e) => log::error!("Error saving user state: {:?}", e.to_string()),
            Ok(output_file) => {
                let groups = self.groups.get_mut();
                for (group_name, group) in groups {
                    self.group_list.replace(group_name.clone());
                    group
                        .mls_group
                        .borrow_mut()
                        .save(self.crypto.key_store())
                        .unwrap();
                }

                self.save_to_file(&output_file);

                match self.crypto.save_keystore(self.user_id.clone()) {
                    Ok(_) => log::info!("User state saved"),
                    Err(e) => log::error!("Error saving user state : {:?}", e.to_string()),
                }
            }
        }
    }
}

#[cfg(target_family = "wasm")]
impl UserStore for User {
    async fn load(user_id: &str) -> Result<User, String> {
        let database = index_db_helper::build_database(user_id).await;
        match database {
            Err(e) => {
                log::error!("Error loading user state: {:?}", e.to_string());
                Err(e.to_string())
            }
            Ok(database) => {
                let user_result = User::load_from_file(&database, user_id).await;
                if user_result.is_ok() {
                    let mut user = user_result.ok().unwrap();
                    match user.crypto.load_keystore(user_id.to_string()).await {
                        Ok(_) => {
                            let groups = user.groups.get_mut();
                            for group_name in &user.group_list {
                                let group = groups.get_mut(group_name).unwrap();
                                let mls_group = MlsGroup::load(
                                    &GroupId::from_slice(group_name.as_bytes()),
                                    user.crypto.key_store(),
                                );
                                let grp = Group {
                                    mls_group: RefCell::new(mls_group.unwrap()),
                                    group_id: group_name.clone(),
                                    conversation: group.conversation.clone(),
                                };
                                groups.insert(group_name.clone(), grp);
                            }
                            Ok(user)
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    user_result
                }
            }
        }
    }

    /// Save the user state to a file.
    async fn save(&mut self) {
        let groups = self.groups.get_mut();
        for (group_name, group) in groups {
            self.group_list.replace(group_name.clone());
            group
                .mls_group
                .borrow_mut()
                .save(self.crypto.key_store())
                .unwrap();
        }

        self.save_to_file().await;

        match self.crypto.save_keystore(self.user_id.clone()).await {
            Ok(_) => log::debug!("User state saved"),
            Err(e) => panic!("Error saving user state : {:?}", e.to_string()),
        }
    }
}

/// Functional interface to the user.
impl User {
    /// Create a new user with the given name and a fresh set of credentials.
    pub fn new(user_id: &str) -> Self {
        let crypto = OpenMlsRustPersistentCrypto::default();
        let out = Self {
            user_id: user_id.to_string(),
            groups: RefCell::new(HashMap::new()),
            group_list: HashSet::new(),
            identity: RefCell::new(Identity::new(CIPHERSUITE, &crypto, user_id.as_bytes())),
            backend: Backend::default(),
            crypto,
            autosave_enabled: false,
            mls_sync_timestamp: RefCell::new(0),
        };
        return out;
    }

    /// Get a list of groups the user is a member of.
    pub fn get_groups(&self) -> Vec<String> {
        self.groups.borrow().keys().cloned().collect()
    }

    ///
    pub fn enable_auto_save(&mut self) {
        self.autosave_enabled = true;
    }

    async fn autosave(&mut self) -> bool {
        if self.autosave_enabled {
            self.save().await;
            return true;
        } else {
            return false;
        }
    }

    /// Add a key package to the user identity and return the pair [key package
    /// hash ref , key package]
    fn add_key_package(&self) -> (Vec<u8>, KeyPackage) {
        let kp = self
            .identity
            .borrow_mut()
            .add_key_package(CIPHERSUITE, &self.crypto);
        (
            kp.hash_ref(self.crypto.crypto())
                .unwrap()
                .as_slice()
                .to_vec(),
            kp,
        )
    }

    /// Get a member
    fn find_member_index(&self, name: String, group: &Group) -> Result<LeafNodeIndex, String> {
        let mls_group = group.mls_group.borrow();
        for Member {
            index,
            encryption_key: _,
            signature_key: _,
            credential,
        } in mls_group.members()
        {
            if credential.identity() == name.as_bytes() {
                return Ok(index);
            }
        }
        Err("Unknown member".to_string())
    }

    /// Get the key packages fo this user.
    pub(crate) fn key_packages(&self) -> Vec<(Vec<u8>, KeyPackage)> {
        // clone first !
        let kpgs = self.identity.borrow().kp.clone();
        Vec::from_iter(kpgs)
    }

    /// Get the key packages fo this user.
    pub(crate) fn key_packages_map(&self) -> HashMap<String, String> {
        let key_packages = self.key_packages();
        let mut key_package_map = HashMap::new();
        for (key, value) in key_packages {
            key_package_map.insert(
                base64::encode_config(key, base64::URL_SAFE),
                base64::encode_config(value.tls_serialize_detached().unwrap(), base64::URL_SAFE),
            );
        }
        return key_package_map;
    }

    ///
    pub async fn publish_key_packages(&self) -> Result<String, String> {
        return self.backend.register_key_packages(self).await;
    }

    pub async fn create_kp_and_publish(&self) -> Result<(), String> {
        let _ = self.add_key_package();
        self.publish_key_packages().await?;
        Ok(())
    }

    /// Return the last 100 messages sent to the group.
    pub fn read_msgs(&self, group_id: String) -> Result<Option<Vec<ConversationMessage>>, String> {
        let groups = self.groups.borrow();
        groups.get(&group_id).map_or_else(
            || Err("Unknown group".to_string()),
            |g| {
                Ok(g.conversation
                    .get(100)
                    .map(|messages: &[ConversationMessage]| messages.to_vec()))
            },
        )
    }

    /// Send an application message to the group.
    pub async fn send_msg(&mut self, msg: &str, group_id: &str) -> Result<String, String> {
        let mut groups = self.groups.borrow_mut();

        let group = match groups.get_mut(group_id) {
            Some(g) => g,
            None => return Err("Unknown group".to_string()),
        };

        let message_out = group
            .mls_group
            .borrow_mut()
            .create_message(&self.crypto, &self.identity.borrow().signer, msg.as_bytes())
            .map_err(|e| format!("{e}"))?;

        let msg_bytes: Vec<u8> = message_out.to_bytes().map_err(|e| format!("{e}"))?;
        log::debug!(" >>> send: {:?}", msg);
        let msg_to_ds = bytes_to_hex(&msg_bytes);

        // cache the message sent by self, since it will cannot be decrypt by self.
        let conversation_message =
            ConversationMessage::new(String::from_str(msg).unwrap(), self.user_id.to_string());
        group
            .conversation
            .borrow_mut()
            .add(conversation_message, Some(&msg_to_ds.clone()));

        drop(groups);
        self.autosave().await;

        Ok(msg_to_ds)
    }

    /// Reads the message, content should be hex encoded.
    pub fn read_msg(&self, content: &str, sender: &str, group_id: &str) -> Result<String, String> {
        match self.groups.borrow_mut().get_mut(group_id) {
            Some(group) => match group.conversation.get_cached_message(content) {
                Some(message) => Ok(message),
                None => {
                    log::debug!("read_msg::Message not found in cache, trying to decrypt");
                    let msg_bytes = hex_to_bytes(&content);
                    let mls_message = MlsMessageIn::tls_deserialize_exact(msg_bytes)
                        .map_err(|_| "Could not deserialize message.".to_string())?;
                    let protocol_message: ProtocolMessage = mls_message.into();
                    // get the MlsGroup from groups
                    match group
                        .mls_group
                        .borrow_mut()
                        .process_message(&self.crypto, protocol_message)
                    {
                        Ok(processed_message) => {
                            if let ProcessedMessageContent::ApplicationMessage(
                                application_message,
                            ) = processed_message.into_content()
                            {
                                // bytes to string
                                let result = String::from_utf8(application_message.into_bytes())
                                    .map_err(|_| "Invalid UTF-8 sequence".to_string());
                                // cache the result
                                if result.is_ok() {
                                    let conversation_message = ConversationMessage::new(
                                        result.clone().unwrap(),
                                        sender.to_string(),
                                    );
                                    group.conversation.add(conversation_message, Some(content));
                                }
                                return result;
                            } else {
                                Err("Error processing unverified message".to_string())
                            }
                        }
                        Err(e) => Err(format!("Could not process message: {}", e)),
                    }
                }
            },
            None => Err("Group not found".to_string()),
        }
    }

    /// Update the user. This involves:
    /// * retrieving all new messages from the server
    /// * update the contacts with all other clients known to the server
    pub async fn update(&mut self, groups: Vec<String>) -> Result<(), String> {
        log::debug!("Updating {} ...", self.user_id);
        log::debug!("update::Processing messages for {} ", self.user_id);
        // Go through the list of messages and process or store them.
        for message in self.backend.recv_mls_events(self, groups).await?.drain(..) {
            log::debug!("Reading message format {:#?} ...", message.wire_format());
            let _ = self.handle_mls_group_event(message).await;
        }

        log::debug!("update::Processing messages done");
        self.autosave().await;

        Ok(())
    }

    pub async fn handle_mls_group_event(&mut self, message: MlsMessageIn) -> Result<(), String> {
        match message.extract() {
            MlsMessageInBody::Welcome(welcome) => {
                // Join the group. (Later we should ask the user to
                // approve first ...)
                self.join_group(welcome).await?;
            }
            MlsMessageInBody::PrivateMessage(message) => {
                match self.process_protocol_message(message.into()).await {
                    Ok(p) => {
                        if p.0 == PostUpdateActions::Remove {
                            match p.1 {
                                Some(gid) => {
                                    let group_id = str::from_utf8(gid.as_slice()).unwrap();
                                    let group_id_to_remove = group_id;
                                    {
                                        let mut grps: std::cell::RefMut<
                                            '_,
                                            HashMap<String, Group>,
                                        > = self.groups.borrow_mut();
                                        grps.remove_entry(group_id_to_remove);
                                    }
                                    self.group_list.remove(group_id_to_remove);
                                }
                                None => log::debug!(
                                    "update::Error post update remove must have a group id"
                                ),
                            }
                        }
                    }
                    Err(_e) => {}
                };
            }
            MlsMessageInBody::PublicMessage(message) => {
                if self.process_protocol_message(message.into()).await.is_err() {}
            }
            _ => panic!("Unsupported message type"),
        }

        Ok(())
    }

    async fn process_protocol_message(
        &mut self,
        message: ProtocolMessage,
    ) -> Result<(PostUpdateActions, Option<GroupId>), String> {
        let message_cloned = message.clone();
        let group_id_clone = str::from_utf8(message_cloned.group_id().as_slice()).unwrap();
        {
            let processed_message: ProcessedMessage;

            let mut groups = self.groups.borrow_mut();

            let group = match groups.borrow_mut().get_mut(group_id_clone) {
                Some(g) => g,
                None => {
                    log::error!(
                        "Error getting group {:?} for a message. Dropping message.",
                        message.group_id()
                    );
                    return Err("error".to_string());
                }
            };
            let mut mls_group = group.mls_group.borrow_mut();
            processed_message = match mls_group.process_message(&self.crypto, message) {
                Ok(msg) => msg,
                Err(e) => {
                    log::error!(
                        "Error processing unverified message: {:?} -  Dropping message.",
                        e
                    );
                    return Err("error".to_string());
                }
            };

            match processed_message.into_content() {
                ProcessedMessageContent::ApplicationMessage(_) => {
                    // intentionally left blank.
                }
                ProcessedMessageContent::ProposalMessage(_proposal_ptr) => {
                    // intentionally left blank.
                }
                ProcessedMessageContent::ExternalJoinProposalMessage(_external_proposal_ptr) => {
                    // intentionally left blank.
                }
                ProcessedMessageContent::StagedCommitMessage(commit_ptr) => {
                    let mut remove_proposal: bool = false;
                    if commit_ptr.self_removed() {
                        remove_proposal = true;
                    }
                    match mls_group.merge_staged_commit(&self.crypto, *commit_ptr) {
                        Ok(()) => {
                            if remove_proposal {
                                log::debug!(
                                "update::Processing StagedCommitMessage removing {} from group {} ",
                                self.user_id,
                                group.group_id
                            );
                                return Ok((
                                    PostUpdateActions::Remove,
                                    Some(mls_group.group_id().clone()),
                                ));
                            }
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                }
            }
        }

        self.update_group_sync_timestamp().await;

        Ok((PostUpdateActions::None, None))
    }

    /// Create a group with the given name.
    pub async fn create_group(&mut self, group_id: &str) -> Result<String, String> {
        log::debug!("{} creates group {}", self.user_id, group_id);
        let group_id_bytes = group_id.as_bytes();
        let mut group_aad = group_id_bytes.to_vec();
        group_aad.extend(b" AAD");

        // NOTE: Since the DS currently doesn't distribute copies of the group's ratchet
        // tree, we need to include the ratchet_tree_extension.
        let group_config = MlsGroupConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();

        let mut mls_group = MlsGroup::new_with_group_id(
            &self.crypto,
            &self.identity.borrow().signer,
            &group_config,
            GroupId::from_slice(group_id_bytes),
            self.identity.borrow().credential_with_key.clone(),
        )
        .expect("Failed to create MlsGroup");

        mls_group.set_aad(group_aad.as_slice());

        let group = Group {
            group_id: group_id.to_string(),
            conversation: Conversation::default(),
            mls_group: RefCell::new(mls_group),
        };

        if self.groups.borrow().contains_key(group_id) {
            panic!("Group '{}' existed already", group_id);
        }

        self.groups.borrow_mut().insert(group_id.to_string(), group);
        self.autosave().await;
        Ok(group_id.to_string())
    }

    /// Check if the user with the given name can be invited to the group.
    pub async fn can_invite(&self, user_id: &str) -> bool {
        let result = self.backend.consume_key_package(user_id).await;
        result.is_ok()
    }

    /// Invite user with the given name to the group.
    pub async fn add_member_to_group(
        &mut self,
        user_id: &str,
        group_name: &str,
    ) -> Result<(), String> {
        // Reclaim a key package from the server
        let joiner_key_package = self.backend.consume_key_package(&user_id).await.unwrap();

        // Build a proposal with this key package and do the MLS bits.
        let mut groups = self.groups.borrow_mut();
        let group = match groups.get_mut(group_name) {
            Some(g) => g,
            None => return Err(format!("No group with name {group_name} known.")),
        };

        let (out_messages, welcome, group_info) = group
            .mls_group
            .borrow_mut()
            .add_members(
                &self.crypto,
                &self.identity.borrow().signer,
                &[joiner_key_package.into()],
            )
            .map_err(|e| format!("Failed to add member to group - {e}"))?;

        /* First, send the MlsMessage commit to the group.
        This must be done before the member invitation is locally committed.
        It avoids the invited member to receive the commit message (which is in the previous group epoch).*/
        log::trace!("Sending commit");
        let group = groups.get_mut(group_name).unwrap(); // XXX: not cool.

        // convert group.group_id to bytes
        let msg = GroupMessage::new(out_messages.into(), &group.group_id);
        self.backend
            .send_msg(&msg, &self.user_id, &group_name)
            .await?;

        // Second, process the invitation on our end.
        group
            .mls_group
            .borrow_mut()
            .merge_pending_commit(&self.crypto)
            .expect("error merging pending commit");

        // publish group info
        match group_info {
            Some(group_info) => {
                self.backend.publish_group_info(&group_info.into()).await?;
            }
            None => {}
        }

        // Finally, send Welcome to the joiner.
        log::trace!("Sending welcome");
        self.backend
            .send_welcome(&welcome, &self.user_id, &user_id, &group_name)
            .await
            .expect("Error sending Welcome message");

        drop(groups);

        self.autosave().await;

        Ok(())
    }

    /// Remove user with the given name from the group.
    pub async fn remove(&mut self, name: String, group_name: String) -> Result<(), String> {
        // Get the group ID

        let mut groups = self.groups.borrow_mut();
        let group = match groups.get_mut(&group_name) {
            Some(g) => g,
            None => return Err(format!("No group with name {group_name} known.")),
        };

        // Get the client leaf index

        let leaf_index = match self.find_member_index(name, group) {
            Ok(l) => l,
            Err(e) => return Err(e),
        };

        // Remove operation on the mls group
        let (remove_message, _welcome, group_info) = group
            .mls_group
            .borrow_mut()
            .remove_members(&self.crypto, &self.identity.borrow().signer, &[leaf_index])
            .map_err(|e| format!("Failed to remove member from group - {e}"))?;

        // First, send the MlsMessage remove commit to the group.
        log::trace!("Sending commit");
        let group = groups.get_mut(&group_name).unwrap(); // XXX: not cool.

        let msg = GroupMessage::new(remove_message.into(), &group.group_id);
        self.backend
            .send_msg(&msg, &self.user_id, &group_name)
            .await?;

        // publish group info
        match group_info {
            Some(group_info) => {
                self.backend.publish_group_info(&group_info.into()).await?;
            }
            None => {}
        }

        // Second, process the removal on our end.
        group
            .mls_group
            .borrow_mut()
            .merge_pending_commit(&self.crypto)
            .expect("error merging pending commit");

        drop(groups);

        self.autosave().await;

        Ok(())
    }

    /// Join a group with the provided welcome message.
    async fn join_group(&mut self, welcome: Welcome) -> Result<(), String> {
        log::debug!("{} joining group ...", self.user_id);
        {
            let mut ident = self.identity.borrow_mut();
            for secret in welcome.secrets().iter() {
                let key_package_hash = &secret.new_member();
                if ident.kp.contains_key(key_package_hash.as_slice()) {
                    ident.kp.remove(key_package_hash.as_slice());
                }
            }
        }

        // NOTE: Since the DS currently doesn't distribute copies of the group's ratchet
        // tree, we need to include the ratchet_tree_extension.
        let group_config = MlsGroupConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();
        let mut mls_group = MlsGroup::new_from_welcome(&self.crypto, &group_config, welcome, None)
            .map_err(|e| e.to_string())?;
        let group_id = mls_group.group_id().to_vec();

        // XXX: Use Welcome's encrypted_group_info field to store group_name.
        let group_name = String::from_utf8(group_id.clone()).unwrap();
        let group_aad = group_name.clone() + " AAD";

        mls_group.set_aad(group_aad.as_bytes());

        let group = Group {
            group_id: group_name.clone(),
            conversation: Conversation::default(),
            mls_group: RefCell::new(mls_group),
        };

        let result = match self.groups.borrow_mut().insert(group_name, group) {
            Some(old) => Err(format!("Overrode the group {:?}", old.group_id)),
            None => Ok(()),
        };

        self.create_kp_and_publish().await?;
        self.autosave().await;

        return result;
    }

    /// Leave a group.
    pub async fn leave_group(&mut self, group_id: String) -> Result<(), String> {
        // Get the group ID
        let mut groups = self.groups.borrow_mut();
        let group = match groups.get_mut(&group_id) {
            Some(g) => g,
            None => return Err(format!("No group with name {group_id} known.")),
        };

        // Remove operation on the mls group
        let queued_message = group
            .mls_group
            .borrow_mut()
            .leave_group(&self.crypto, &self.identity.borrow().signer)
            .map_err(|e| format!("Failed to remove member from group - {e}"))?;

        // First, send the MlsMessage remove commit to the group.
        log::trace!("Sending commit");

        let msg = GroupMessage::new(queued_message.into(), &group.group_id);
        self.backend
            .send_msg(&msg, &self.user_id, &group_id)
            .await?;

        // publish group info
        let group_info = group
            .mls_group
            .borrow()
            .export_group_info(self.crypto.crypto(), &self.identity.borrow().signer, true)
            .expect("Error exporting group info");

        // Second, process the removal on our end.
        group
            .mls_group
            .borrow_mut()
            .merge_pending_commit(&self.crypto)
            .expect("error merging pending commit");

        self.backend.publish_group_info(&group_info.into()).await?;

        drop(groups);

        self.autosave().await;

        Ok(())
    }

    ///
    pub async fn update_group_sync_timestamp(&mut self) {
        let timestamp = instant::SystemTime::now()
            .duration_since(instant::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        *self.mls_sync_timestamp.borrow_mut() = timestamp;
        self.autosave().await;
    }

    pub async fn join_group_externally(&mut self, group_id: &str) -> Result<(), String> {
        let group_info = self.backend.get_group_info(group_id).await?;
        return self.join_by_external_commit(group_info).await;
    }

    ///
    async fn join_by_external_commit(
        &mut self,
        group_info: VerifiableGroupInfo,
    ) -> Result<(), String> {
        {
            // join by external commit
            let group_config = MlsGroupConfig::builder()
                .use_ratchet_tree_extension(true)
                .build();
            let credential_key = &self.identity.borrow().credential_with_key;
            let (mut mls_group, out_message, group_info) = MlsGroup::join_by_external_commit(
                &self.crypto,
                &self.identity.borrow().signer,
                None,
                group_info,
                &group_config,
                &[],
                credential_key.to_owned(),
            )
            .expect("Error joining group by external commit");

            // set the aad
            let group_id = mls_group.group_id().to_vec();
            let group_name = String::from_utf8(group_id.clone()).unwrap();
            let group_aad = group_name.clone() + " AAD";
            mls_group.set_aad(group_aad.as_bytes());

            let group = Group {
                group_id: group_name.clone(),
                conversation: Conversation::default(),
                mls_group: RefCell::new(mls_group),
            };

            let result = match self.groups.borrow_mut().insert(group_name.clone(), group) {
                Some(old) => Err(format!("Overrode the group {:?}", old.group_id)),
                None => Ok(()),
            };

            // fan out the out_message
            let msg: GroupMessage = GroupMessage::new(out_message.into(), &group_name);
            self.backend
                .send_msg(&msg, &self.user_id, &group_name)
                .await?;

            // update the group_info
            match group_info {
                Some(group_info) => {
                    self.backend.publish_group_info(&group_info.into()).await?;
                }
                None => {}
            }

            // merge pending commit
            self.groups
                .borrow_mut()
                .get_mut(&group_name)
                .unwrap()
                .mls_group
                .borrow_mut()
                .merge_pending_commit(&self.crypto)
                .expect("Error merging pending commit");

            //
            self.create_kp_and_publish().await?;

            if result.is_err() {
                return result;
            }
        }

        self.autosave().await;

        Ok(())
    }

    ///
    pub fn reset_ds_url(&mut self, ds_url: &str) {
        self.backend.reset_ds_url(ds_url);
    }
}
