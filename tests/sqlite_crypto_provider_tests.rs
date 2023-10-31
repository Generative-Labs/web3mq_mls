// use web3mq_mls::db_cipher::DbCipher;
// use web3mq_mls::sqlite_crypto_provider::SqliteCryptoProvider;
// use rusqlite::Connection;

#[cfg(test)]
mod sqlite_crypto_provider_tests {
    #[test]
    fn test_read_by_id() {
        // let cipher = DbCipher::new(&chacha20poly1305::Key::from_slice(&[0, 32]));
        //
        // let db_cipher = DbCipher::new(&chacha20poly1305::Key::from_slice(&[0, 32]));
        // let db = Connection::open(&"").expect("Connect to DB");
        // let provider = SqliteCryptoProvider::new(&db_cipher, db, &RustCrypto::default());
    }

    #[test]
    fn test_write_to_id() {}

    #[test]
    fn test_delete_by_id() {}
}
