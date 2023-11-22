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

pub async fn build_database(user_id: String, database_type: DatabaseType) -> rexie::Result<Rexie> {
    // Create a new database
    Rexie::builder(&user_id.clone())
        // Set the version of the database to 1.0
        .version(1)
        // Add an object store named `ks`
        .add_object_store(
            ObjectStore::new(&database_type.store_name())
                // Set the key path to `id`
                .key_path("id")
                // Enable auto increment
                .auto_increment(true)
                // Add an index named `email` with the key path `email` with unique enabled
                .add_index(Index::new("user_id", "user_id").unique(true)),
        )
        // Build the database
        .build()
        .await
}
