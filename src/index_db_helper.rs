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
