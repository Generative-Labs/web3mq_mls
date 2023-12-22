use openmls_rust_crypto::RustCrypto;
use openmls_traits::OpenMlsProvider;

use super::persistent_key_store::PersistentKeyStore;

#[derive(Default, Debug)]
pub struct OpenMlsRustPersistentCrypto {
    crypto: RustCrypto,
    key_store: PersistentKeyStore,
}

impl OpenMlsProvider for OpenMlsRustPersistentCrypto {
    type CryptoProvider = RustCrypto;
    type RandProvider = RustCrypto;
    type KeyStoreProvider = PersistentKeyStore;

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }

    fn key_store(&self) -> &Self::KeyStoreProvider {
        &self.key_store
    }
}

#[cfg(target_family = "wasm")]
impl OpenMlsRustPersistentCrypto {
    pub async fn save_keystore(&self, user_name: String) -> Result<(), String> {
        self.key_store.save(user_name).await
    }

    pub async fn load_keystore(&mut self, user_name: String) -> Result<(), String> {
        self.key_store.load(user_name).await
    }
}

#[cfg(not(target_family = "wasm"))]
impl OpenMlsRustPersistentCrypto {
    pub fn save_keystore(&self, user_name: String) -> Result<(), String> {
        self.key_store.save(user_name)
    }

    pub fn load_keystore(&mut self, user_name: String) -> Result<(), String> {
        self.key_store.load(user_name)
    }
}
