use std::{collections::HashMap, sync::Mutex};

use lazy_static::lazy_static;
use rexie::{self, Index, ObjectStore, Rexie};

pub enum DatabaseType {
    User,
    KeyStore,
}

impl DatabaseType {
    pub fn store_name(&self) -> String {
        match self {
            DatabaseType::User => "USER".to_string(),
            DatabaseType::KeyStore => "KS".to_string(),
        }
    }
}

// pub struct IndexDBStorage {
//     pub db: Mutex<HashMap<String, Rexie>>,
// }

// lazy_static! {
//     static ref DATABASES: IndexDBStorage = IndexDBStorage {
//         db: Mutex::new(HashMap::new())
//     };
// }

// impl IndexDBStorage {}

// pub async fn get_database(&self, user_id: &str) -> rexie::Result<Rexie> {
//     let mut databases = self.db.lock().unwrap();
//     if let Some(database) = databases.get(user_id) {
//         Ok(database)
//     } else {
//         let database = self.build_database(user_id.to_string()).await?;
//         databases.insert(user_id.to_string(), database);
//         Ok(database)
//     }
// }

pub async fn build_database(user_id: &str) -> rexie::Result<Rexie> {
    // Create a new database
    let database_name = "web3mq_mls_".to_string() + &user_id;
    Rexie::builder(&database_name)
        // Set the version of the database to 1.0
        .version(2)
        // Add an object store named `ks`
        .add_object_store(
            ObjectStore::new(&DatabaseType::User.store_name())
                // Add an index named `user_id` with the key path `user_id` with unique enabled
                .add_index(Index::new("user_id", "user_id").unique(true)),
        )
        .add_object_store(
            ObjectStore::new(&DatabaseType::KeyStore.store_name())
                // Add an index named `user_id` with the key path `user_id` with unique enabled
                .add_index(Index::new("user_id", "user_id").unique(true)),
        )
        // Build the database
        .build()
        .await
}

// impl IndexDBStorage {
//     pub fn instance() -> &'static IndexDBStorage {
//         &SINGLETON
//     }

//     pub async fn getInstance(user_id: &str) -> rexie::Result<Rexie> {
//         self.db
//             .get_or_insert_with(|| self.build_database(user_id).await)
//     }

// }
