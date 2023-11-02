const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct User {
    pub(crate) user_id: String,
    #[serde(
        serialize_with = "serialize_any_hashmap::serialize_hashmap",
        deserialize_with = "serialize_any_hashmap::deserialize_hashmap"
    )]
    #[serde(skip)]
    pub(crate) groups: RefCell<HashMap<String, Group>>,
    group_list: HashSet<String>,
    pub(crate) identity: RefCell<Identity>,
    #[serde(skip)]
    backend: Backend,
    #[serde(skip)]
    crypto: OpenMlsRustPersistentCrypto,
    autosave_enabled: bool,
}

impl User {
    /// Create a new user with the given name and a fresh set of credentials.
    pub fn new(user_id: String) -> Self {
        let crypto = OpenMlsRustPersistentCrypto::default();
        let out = Self {
            user_id: user_id.clone(),
            groups: RefCell::new(HashMap::new()),
            group_list: HashSet::new(),
            identity: RefCell::new(Identity::new(CIPHERSUITE, &crypto, username.as_bytes())),
            backend: Backend::default(),
            crypto,
            autosave_enabled: false,
        };
        out
    }
}
