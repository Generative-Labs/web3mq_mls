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
pub async fn initial_user(user_id: &str) -> Result<(), String> {
    let loaded_user = User::load(user_id).await;
    // if loaded_user isNone, then create a new user
    // and save it to the file system
    return if loaded_user.is_err() {
        let mut user = User::new(user_id);
        user.enable_auto_save();
        user.register().await?;
        user.save().await;
        Ok(())
    } else {
        Ok(())
    };
}

#[wasm_bindgen]
pub async fn register_user(user_id: &str) -> Result<String, String> {
    let user = User::load(user_id).await?;
    return user.register().await;
}

#[wasm_bindgen]
pub async fn is_mls_group(user_id: &str, group_id: &str) -> Result<bool, String> {
    let user = User::load(user_id).await?;
    let groups = user.groups.borrow();
    Ok(groups.contains_key(group_id))
}

#[wasm_bindgen]
pub async fn create_group(user_id: &str, group_id: &str) -> Result<String, String> {
    let mut user = User::load(user_id).await?;
    return user.create_group(group_id).await;
}

#[wasm_bindgen]
pub async fn sync_mls_state(user_id: &str, group_ids: Vec<String>) -> Result<(), String> {
    let mut user = User::load(user_id).await?;
    let _ = user.update(group_ids).await?;
    Ok(())
}

#[wasm_bindgen]
pub async fn can_add_member_to_group(user_id: &str, target_user_id: &str) -> Result<bool, String> {
    let user = User::load(user_id).await?;
    let can_invite = user.can_invite(&target_user_id).await;
    return Ok(can_invite);
}

#[wasm_bindgen]
pub async fn add_member_to_group(
    user_id: &str,
    member_user_id: &str,
    group_id: &str,
) -> Result<(), String> {
    let mut user = User::load(user_id).await?;
    return user.add_member_to_group(&member_user_id, group_id).await;
}

#[wasm_bindgen]
pub async fn mls_encrypt_msg(user_id: &str, msg: &str, group_id: &str) -> Result<String, String> {
    let mut user = User::load(user_id).await?;
    return user.send_msg(&msg, group_id).await;
}

#[wasm_bindgen]
pub async fn mls_decrypt_msg(
    user_id: &str,
    msg: &str,
    sender_user_id: &str,
    group_id: &str,
) -> Result<String, String> {
    let mut user = User::load(user_id).await?;
    let result = user.read_msg(msg, sender_user_id, group_id);
    user.save().await;
    return result;
}

#[wasm_bindgen]
pub async fn handle_mls_group_event(user_id: &str, msg_bytes: Vec<u8>) -> Result<(), String> {
    let mut user = User::load(user_id).await?;
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
    // use openmls::{
    //     credentials::{Credential, CredentialType, CredentialWithKey},
    //     framing::{MlsMessageIn, MlsMessageInBody},
    //     group::{config::CryptoConfig, MlsGroup, MlsGroupConfig},
    //     prelude_test::KeyPackage,
    //     versions::ProtocolVersion,
    // };
    use openmls_basic_credential::SignatureKeyPair;
    use openmls_rust_crypto::OpenMlsRustCrypto;
    use openmls_traits::types::{Ciphersuite, SignatureScheme};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use openmls::prelude::{config::CryptoConfig, *};

    use crate::{
        add_member_to_group, create_group, initial_user,
        service::{
            backend::{self, Backend},
            networking::ed25519_sign,
            user::User,
        },
        setup_networking_config,
    };

    #[tokio::test]
    async fn test_key_packages() {
        // let user_a_id = "user_a".to_string();
        // let user_b_id = "user_b".to_string();
        // let group_id = "test_group_a".to_string();
        // let _ = initial_user(user_a_id.clone()).await;
        // let _ = initial_user(user_b_id.clone()).await;
        // let _ = create_group(user_a_id.clone(), group_id.clone()).await;
        // let _ = add_member_to_group(user_a_id.clone(), user_b_id.clone(), group_id).await;

        // Define ciphersuite ...
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        // ... and the crypto backend to use.
        let sasha_backend = &OpenMlsRustCrypto::default();
        let bob_backend = &OpenMlsRustCrypto::default();
        let maxim_backend = &OpenMlsRustCrypto::default();

        // First they need credentials to identify them
        let (sasha_credential_with_key, sasha_signer) = generate_credential_with_key(
            "Sasha".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            sasha_backend,
        );
        let sasha_key_package = generate_key_package(
            ciphersuite,
            sasha_backend,
            &sasha_signer,
            sasha_credential_with_key.clone(),
        );

        let (bob_credential_with_key, bob_signer) = generate_credential_with_key(
            "Bob".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            sasha_backend,
        );

        let bob_key_package = generate_key_package(
            ciphersuite,
            bob_backend,
            &bob_signer,
            bob_credential_with_key.clone(),
        );

        let (maxim_credential_with_key, maxim_signer) = generate_credential_with_key(
            "Maxim".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            maxim_backend,
        );

        let maxim_key_package = generate_key_package(
            ciphersuite,
            maxim_backend,
            &maxim_signer,
            maxim_credential_with_key.clone(),
        );

        let group_config = MlsGroupConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();

        let mut sasha_group = MlsGroup::new(
            sasha_backend,
            &sasha_signer,
            &group_config,
            sasha_credential_with_key.clone(),
        )
        .expect("An unexpected error occurred.");

        let (mls_message_out, welcome_out, group_info) = sasha_group
            .add_members(sasha_backend, &sasha_signer, &[bob_key_package.clone()])
            .expect("Could not add members.");

        sasha_group
            .merge_pending_commit(sasha_backend)
            .expect("error merging pending commit");

        // Sascha serializes the [`MlsMessageOut`] containing the [`Welcome`].
        let serialized_welcome = welcome_out
            .tls_serialize_detached()
            .expect("Error serializing welcome");

        let mls_message_in = MlsMessageIn::tls_deserialize(&mut serialized_welcome.as_slice())
            .expect("An unexpected error occurred.");
        // ... and inspect the message.
        let welcome = match mls_message_in.extract() {
            MlsMessageInBody::Welcome(welcome) => welcome,
            // We know it's a welcome message, so we ignore all other cases.
            _ => unreachable!("Unexpected message type."),
        };

        // Now Maxim can join the group.
        let mut bob_group = MlsGroup::new_from_welcome(bob_backend, &group_config, welcome, None)
            .expect("Error joining group from Welcome");

        let mut sasha_group_2 = MlsGroup::new(
            sasha_backend,
            &sasha_signer,
            &group_config,
            sasha_credential_with_key.clone(),
        )
        .expect("An unexpected error occurred.");

        // let bob_key_package = generate_key_package(
        //     ciphersuite,
        //     bob_backend,
        //     &bob_signer,
        //     bob_credential_with_key.clone(),
        // );

        let (mls_message_out, welcome_out, group_info) = sasha_group_2
            .add_members(sasha_backend, &sasha_signer, &[bob_key_package])
            .expect("Could not add members.");

        sasha_group_2
            .merge_pending_commit(sasha_backend)
            .expect("error merging pending commit");

        // Sascha serializes the [`MlsMessageOut`] containing the [`Welcome`].
        let serialized_welcome = welcome_out
            .tls_serialize_detached()
            .expect("Error serializing welcome");

        let mls_message_in = MlsMessageIn::tls_deserialize(&mut serialized_welcome.as_slice())
            .expect("An unexpected error occurred.");
        // ... and inspect the message.
        let welcome = match mls_message_in.extract() {
            MlsMessageInBody::Welcome(welcome) => welcome,
            // We know it's a welcome message, so we ignore all other cases.
            _ => unreachable!("Unexpected message type."),
        };

        let mut bob_group = MlsGroup::new_from_welcome(bob_backend, &group_config, welcome, None)
            .expect("Error joining group from Welcome");
    }

    #[tokio::test]
    async fn test_external_join_group() {
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        // ... and the crypto backend to use.
        let sasha_backend = &OpenMlsRustCrypto::default();
        let bob_backend = &OpenMlsRustCrypto::default();

        let (sasha_credential_with_key, sasha_signer) = generate_credential_with_key(
            "Sasha".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            sasha_backend,
        );

        let group_config = MlsGroupConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();
        let mut sasha_group = MlsGroup::new(
            sasha_backend,
            &sasha_signer,
            &group_config,
            sasha_credential_with_key.clone(),
        )
        .expect("An unexpected error occurred.");

        let (bob_credential_with_key, bob_signer) = generate_credential_with_key(
            "Bob".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            bob_backend,
        );
        let bob_key_package = generate_key_package(
            ciphersuite,
            bob_backend,
            &bob_signer,
            bob_credential_with_key.clone(),
        );

        let proposal = JoinProposal::new(
            bob_key_package,
            sasha_group.group_id().clone(),
            sasha_group.epoch(),
            &bob_signer,
        )
        .expect("Could not create external Add proposal");

        let sasha_processed_message = sasha_group
            .process_message(
                sasha_backend,
                proposal
                    .into_protocol_message()
                    .expect("Unexpected message type."),
            )
            .expect("Could not process message.");

        match sasha_processed_message.into_content() {
            ProcessedMessageContent::ExternalJoinProposalMessage(proposal) => {
                sasha_group.store_pending_proposal(*proposal);
                let (_commit, welcome, _group_info) = sasha_group
                    .commit_to_pending_proposals(sasha_backend, &sasha_signer)
                    .expect("Could not commit");
                assert_eq!(sasha_group.members().count(), 1);
                sasha_group
                    .merge_pending_commit(sasha_backend)
                    .expect("Could not merge commit");
                assert_eq!(sasha_group.members().count(), 2);

                let bob_group = MlsGroup::new_from_welcome(
                    bob_backend,
                    &group_config,
                    welcome
                        .unwrap()
                        .into_welcome()
                        .expect("Unexpected message type."),
                    None,
                )
                .expect("Bob could not join the group");
                assert_eq!(bob_group.members().count(), 2);
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn test_external_commit() {
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        // ... and the crypto backend to use.
        let sasha_backend = &OpenMlsRustCrypto::default();
        let bob_backend = &OpenMlsRustCrypto::default();

        // First they need credentials to identify them
        let (sasha_credential_with_key, sasha_signer) = generate_credential_with_key(
            "Sasha".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            sasha_backend,
        );
        let sasha_key_package = generate_key_package(
            ciphersuite,
            sasha_backend,
            &sasha_signer,
            sasha_credential_with_key.clone(),
        );

        let (bob_credential_with_key, bob_signer) = generate_credential_with_key(
            "Bob".into(),
            CredentialType::Basic,
            ciphersuite.signature_algorithm(),
            sasha_backend,
        );

        let group_config = MlsGroupConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();

        let mut sasha_group = MlsGroup::new(
            sasha_backend,
            &sasha_signer,
            &group_config,
            sasha_credential_with_key.clone(),
        )
        .expect("An unexpected error occurred.");

        // sasha_group.export_group_info(crypto, signer, with_ratchet_tree)

        let group_info = sasha_group
            .export_group_info(sasha_backend.crypto(), &sasha_signer, true)
            .expect("Error exporting group info");

        let verifiable_group_info = group_info.into_verifiable_group_info();

        let result = MlsGroup::join_by_external_commit(
            bob_backend,
            &bob_signer,
            None,
            verifiable_group_info.unwrap(),
            &group_config,
            &[],
            bob_credential_with_key,
        )
        .expect("Error joining group by external commit");

        // print sasha_group.aad()
        print!("sasha group aad: {:?}", sasha_group.aad());

        let (bob_group, message_out, group_info) = result;

        print!("bob group aad: {:?}", bob_group.aad());

        // print bob group id
        print!("bob group id: {:?}", bob_group.group_id());
    }

    #[tokio::test]
    async fn test_persistent() {
        let user_id = "Alice".to_string();
        let loaded_user = User::load(&user_id).await;
        // if loaded_user isNone, then create a new user
        // and save it to the file system
        if loaded_user.is_err() {
            let mut user = User::new(&user_id);
            user.enable_auto_save();
            user.save().await;
            print!("user created")
        } else {
            print!("user already exists")
        }
    }

    #[tokio::test]
    async fn test_http_request_signer() {
        let private_key = "5111ec7fda1046fa8a4bfcd8351307068c92f4932b81015d3e32a93efa5fe824";
        let user_id = "user:ea63cbd115dc2a4a2935f6ee669725c11ac2638fa5200ba94d71c84a";
        let timestamp: u128 = 1701400968312;
        let sign_content = user_id.to_string() + &timestamp.to_string();
        let web3mq_signature = ed25519_sign(private_key, &sign_content);
        let result_should_be = "zUX58rJn5c9e4e3pwni2M0wl5D6w5z9iUtUbJBAU5P7ltYDoPxQLhd0BEBbhwrXs/grT8caP1abvIR7lrCiQDA==";
        assert_eq!(web3mq_signature.unwrap(), result_should_be.to_string());
    }

    #[tokio::test]
    async fn test_sha256() {
        let mut hasher = Sha256::new();
        let content = "hello world";
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        let hex = hex::encode(result);
        assert_eq!(
            hex,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        let hex2 = Sha256::digest(content.as_bytes())
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        assert_eq!(
            hex2,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[tokio::test]
    async fn test_payload_hash() {
        let groups = [
            "group:3a2d37237eaf7d60326a88b5c7cd25e78e303458",
            "group:27bbdfff52c2347e93acca128da54fe886c68bbb",
            "group:76694787cfc4b73131eff0db911653e5d0e9def7",
            "group:ac696835d58f879e20f8cbdf2e00008184aa2d12",
            "group:1d5cffe671748927a15d29b377879c99cb17f1bc",
            "group:187918ac3cdc68c06ed532e113eb717ba50fd5aa",
            "group:d75724130864b3a1aaa8b9c8196da3ec7017b495",
            "group:da034f7b852c22df502f6d29008936665ea0f9cb",
            "group:68b2311f81bc3999f048df2c8d80c7321039d679",
            "group:8bec812b6c744a181adeeb28510ed6df4b9055b6",
            "group:4907dc8e43612130c09b5c9bec08f6649d840926",
            "group:ea8b41ea35c0cc8efd4b5a6f598d54f2ed890e3a",
            "group:4a9682ccd3a32fa8d33ec095ded011d0503bb246",
            "group:c19a92a37b47a04c83c529693c9f8b1aae3ba210",
            "group:e2be6645f361bed895e667b5a32e283a9d930caf",
            "group:fa19c6e2b690a3276b593a26aa4b7808ae168de3",
            "group:be11c77d49abc55d832a08ec3c955d89f817f279",
            "group:6b55b4c58374384fbc87bc7354397db18db3c063",
            "group:5165a2d37966179181cd05189667c74bd7876cbd",
            "group:9d5d00393812fcc3b55bc1c93bff2bcf536ea76c",
            "group:f414d0115d343ac383aa999f679b41416d7658e7",
            "group:9a1e7dedcf003a90ee825393a7d0662729ba9939",
            "group:de12398fce5b740b7741e36e6694cfea677c6842",
            "group:66f1f6428d8899d47b97cb393e1b2250ea9929af",
            "group:8c93e3a1e00ece72a91595e412267290c18f243d",
            "group:1d2302b0b38c58245bcfa2c3d32042e8f0963c0d",
        ];
        let groups_json = serde_json::to_string(&groups).unwrap();
        print!("groups_json: {:?}", groups_json);
        let body = base64::encode(groups_json.clone());
        print!("body: {:?}", body);
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

        let user = User::new("Alice");

        let backend = Backend::default();
        backend.register_key_packages(&user).await;

        // user.register().await;

        hello_mock.assert();
    }

    #[tokio::test]
    async fn test_consume_key_package() {
        let user_id = "Alice";
        let mut user = User::new(user_id);

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
        let user_id = "Alice";
        let mut user = User::new(user_id);

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

    // Now let's create two participants.

    // A helper to create and store credentials.
    fn generate_credential_with_key(
        identity: Vec<u8>,
        credential_type: CredentialType,
        signature_algorithm: SignatureScheme,
        backend: &impl OpenMlsProvider,
    ) -> (CredentialWithKey, SignatureKeyPair) {
        let credential = Credential::new(identity, credential_type).unwrap();
        let signature_keys = SignatureKeyPair::new(signature_algorithm)
            .expect("Error generating a signature key pair.");

        // Store the signature key into the key store so OpenMLS has access
        // to it.
        signature_keys
            .store(backend.key_store())
            .expect("Error storing signature keys in key store.");

        (
            CredentialWithKey {
                credential,
                signature_key: signature_keys.public().into(),
            },
            signature_keys,
        )
    }

    // A helper to create key package bundles.
    fn generate_key_package(
        ciphersuite: Ciphersuite,
        backend: &impl OpenMlsProvider,
        signer: &SignatureKeyPair,
        credential_with_key: CredentialWithKey,
    ) -> KeyPackage {
        // Create the key package
        KeyPackage::builder()
            .build(
                CryptoConfig {
                    ciphersuite,
                    version: ProtocolVersion::default(),
                },
                backend,
                signer,
                credential_with_key,
            )
            .unwrap()
    }
}
