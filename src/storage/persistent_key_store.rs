use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
    sync::RwLock,
};

use openmls_traits::key_store::{MlsEntity, OpenMlsKeyStore};
use serde::{Deserialize, Serialize};

use crate::file_helpers;

#[derive(Debug, Default)]
pub struct PersistentKeyStore {
    values: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SerializableKeyStore {
    values: HashMap<String, String>,
}

impl OpenMlsKeyStore for PersistentKeyStore {
    /// The error type returned by the [`OpenMlsKeyStore`].
    type Error = PersistentKeyStoreError;

    /// Store a value `v` that implements the [`ToKeyStoreValue`] trait for
    /// serialization for ID `k`.
    ///
    /// Returns an error if storing fails.
    fn store<V: MlsEntity>(&self, k: &[u8], v: &V) -> Result<(), Self::Error> {
        let value =
            serde_json::to_vec(v).map_err(|_| PersistentKeyStoreError::SerializationError)?;
        // We unwrap here, because this is the only function claiming a write
        // lock on `credential_bundles`. It only holds the lock very briefly and
        // should not panic during that period.
        let mut values = self.values.write().unwrap();
        values.insert(k.to_vec(), value);
        Ok(())
    }

    /// Read and return a value stored for ID `k` that implements the
    /// [`FromKeyStoreValue`] trait for deserialization.
    ///
    /// Returns [`None`] if no value is stored for `k` or reading fails.
    fn read<V: MlsEntity>(&self, k: &[u8]) -> Option<V> {
        // We unwrap here, because the two functions claiming a write lock on
        // `init_key_package_bundles` (this one and `generate_key_package_bundle`) only
        // hold the lock very briefly and should not panic during that period.
        let values = self.values.read().unwrap();
        if let Some(value) = values.get(k) {
            serde_json::from_slice(value).ok()
        } else {
            None
        }
    }

    /// Delete a value stored for ID `k`.
    ///
    /// Returns an error if storing fails.
    fn delete<V: MlsEntity>(&self, k: &[u8]) -> Result<(), Self::Error> {
        // We just delete both ...
        let mut values = self.values.write().unwrap();
        values.remove(k);
        Ok(())
    }
}

#[cfg(target_family = "wasm")]
impl PersistentKeyStore {
    async fn save_to_file(&self, user_id: String) -> Result<(), String> {
        // map the error to String
        let mut ser_ks = SerializableKeyStore::default();
        for (key, value) in &*self.values.read().unwrap() {
            ser_ks.values.insert(
                base64::encode_config(key, base64::URL_SAFE),
                base64::encode_config(value, base64::URL_SAFE),
            );
        }

        let database = index_db_helper::build_database(&user_id)
            .await
            .map_err(|e| e.to_string())?;

        let transaction = database
            .transaction(
                &[DatabaseType::KeyStore.store_name()],
                TransactionMode::ReadWrite,
            )
            .map_err(|e| e.to_string())?;

        let store = transaction
            .store(DatabaseType::KeyStore.store_name().as_str())
            .map_err(|e| e.to_string())?;

        let ks = serde_wasm_bindgen::to_value(&ser_ks).unwrap();
        let key = serde_wasm_bindgen::to_value(&user_id.clone()).unwrap();
        store
            .put(&ks, Some(&key))
            .await
            .map_err(|e| e.to_string())?;
        transaction.done().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn save(&self, user_name: String) -> Result<(), String> {
        self.save_to_file(user_name.clone())
            .await
            .map_err(|e| e.to_string())
    }

    async fn load_from_file(&mut self, user_id: String) -> Result<(), String> {
        // Read the JSON contents of the file as an instance of `SerializableKeyStore`.

        let database = index_db_helper::build_database(&user_id)
            .await
            .map_err(|e| e.to_string())?;
        let transaction = database
            .transaction(
                &[DatabaseType::KeyStore.store_name()],
                TransactionMode::ReadOnly,
            )
            .map_err(|e| e.to_string())?;

        let store = transaction
            .store(DatabaseType::KeyStore.store_name().as_str())
            .map_err(|e| e.to_string())?;

        let user_key_store = store
            .get(&user_id.into())
            .await
            .map_err(|e| e.to_string())?;

        transaction.done().await.map_err(|e| e.to_string())?;

        let serializable_key_store: Option<SerializableKeyStore> =
            serde_wasm_bindgen::from_value(user_key_store).unwrap();

        match serializable_key_store {
            Some(ser_ks) => {
                let mut ks_map = self.values.write().unwrap();
                for (key, value) in ser_ks.values {
                    ks_map.insert(
                        base64::decode_config(key, base64::URL_SAFE).unwrap(),
                        base64::decode_config(value, base64::URL_SAFE).unwrap(),
                    );
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn load(&mut self, user_name: String) -> Result<(), String> {
        self.load_from_file(user_name)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(target_family = "wasm"))]
impl PersistentKeyStore {
    fn get_file_path(user_name: &String) -> PathBuf {
        file_helpers::get_file_path(&("web3mq_".to_owned() + user_name + "_ks.json"))
    }

    fn save_to_file(&self, output_file: &File) -> Result<(), String> {
        let writer = BufWriter::new(output_file);

        let mut ser_ks = SerializableKeyStore::default();
        for (key, value) in &*self.values.read().unwrap() {
            ser_ks
                .values
                .insert(base64::encode(key), base64::encode(value));
        }

        match serde_json::to_writer_pretty(writer, &ser_ks) {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn save(&self, user_name: String) -> Result<(), String> {
        let ks_output_path = PersistentKeyStore::get_file_path(&user_name);

        match File::create(ks_output_path) {
            Ok(output_file) => self.save_to_file(&output_file),
            Err(e) => Err(e.to_string()),
        }
    }

    fn load_from_file(&mut self, input_file: &File) -> Result<(), String> {
        // Prepare file reader.
        let reader = BufReader::new(input_file);

        // Read the JSON contents of the file as an instance of `SerializableKeyStore`.
        match serde_json::from_reader::<BufReader<&File>, SerializableKeyStore>(reader) {
            Ok(ser_ks) => {
                let mut ks_map = self.values.write().unwrap();
                for (key, value) in ser_ks.values {
                    ks_map.insert(base64::decode(key).unwrap(), base64::decode(value).unwrap());
                }
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn load(&mut self, user_name: String) -> Result<(), String> {
        let ks_input_path = PersistentKeyStore::get_file_path(&user_name);

        match File::open(ks_input_path) {
            Ok(input_file) => self.load_from_file(&input_file),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Errors thrown by the key store.
#[derive(thiserror::Error, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PersistentKeyStoreError {
    #[error("Error serializing value.")]
    SerializationError,
}
