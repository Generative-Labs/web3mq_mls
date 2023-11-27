use std::collections::{HashMap, HashSet};

use openmls::prelude::*;
use tls_codec::{
    TlsByteSliceU16, TlsByteVecU16, TlsByteVecU32, TlsByteVecU8, TlsDeserialize, TlsSerialize,
    TlsSize, TlsVecU32,
};

#[derive(Debug, Default, Clone)]
pub struct ClientInfo {
    pub client_name: String,
    pub key_packages: ClientKeyPackages,
    /// map of reserved key_packages [group_id, key_package_hash]
    pub reserved_key_pkg_hash: HashSet<Vec<u8>>,
    pub id: Vec<u8>,
    pub msgs: Vec<MlsMessageIn>,
    pub welcome_queue: Vec<MlsMessageIn>,
}

#[derive(Debug, Default, Clone)]
pub struct RegisterClientParams {
    pub userid: String,
    pub timestamp: u64,
    pub key_packages: TlsVecU32<(Vec<u8>, Vec<u8>)>,
    pub payload_hash: String,
    pub web3mq_user_mainkey_signature: String,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegisterKeyPackageParams {
    pub userid: String,
    pub timestamp: u128,
    pub key_packages: HashMap<String, String>,
    pub payload_hash: String,
    pub web3mq_user_signature: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct KeyPackagesResult {
    pub(crate) userid: String,
    pub(crate) timestamp: u128,
    pub(crate) web3mq_user_signature: String,
    pub(crate) key_packages: HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct Response<Data> {
    pub(crate) code: u32,
    pub(crate) msg: String,
    pub(crate) data: Data,
}

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    TlsSerialize,
    TlsDeserialize,
    TlsSize,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ClientKeyPackages(pub TlsVecU32<(TlsByteVecU8, KeyPackageIn)>);

impl ClientInfo {
    /// Create a new `ClientInfo` struct for a given client name and vector of
    /// key packages with corresponding hashes.
    pub fn new(client_name: String, mut key_packages: Vec<(Vec<u8>, KeyPackageIn)>) -> Self {
        let key_package: KeyPackage = KeyPackage::from(key_packages[0].1.clone());
        let id = key_package.leaf_node().credential().identity().to_vec();
        Self {
            client_name,
            key_packages: ClientKeyPackages(
                key_packages
                    .drain(..)
                    .map(|(e1, e2)| (e1.into(), e2))
                    .collect::<Vec<(TlsByteVecU8, KeyPackageIn)>>()
                    .into(),
            ),
            reserved_key_pkg_hash: HashSet::new(),
            id: id,
            msgs: Vec::new(),
            welcome_queue: Vec::new(),
        }
    }

    /// The identity of a client is defined as the identity of the first key
    /// package right now.
    pub fn id(&self) -> &[u8] {
        self.id.as_slice()
    }

    /// Acquire a key package from the client's key packages
    /// Mark the key package hash ref as "reserved key package"
    /// The reserved hash ref will be used in DS::send_welcome and removed once welcome is distributed
    pub fn consume_kp(&mut self) -> Result<KeyPackageIn, String> {
        if self.key_packages.0.len() <= 1 {
            // We keep one keypackage to handle ClientInfo serialization/deserialization issues
            return Err("No more keypackage available".to_string());
        }
        match self.key_packages.0.pop() {
            Some(c) => {
                self.reserved_key_pkg_hash.insert(c.0.into_vec());
                Ok(c.1)
            }
            None => Err("No more keypackage available".to_string()),
        }
    }
}

/// An core group message.
/// This is an `MLSMessage` plus the list of recipients as a vector of client
/// names.
#[derive(Debug)]
pub struct GroupMessage {
    pub msg: MlsMessageIn,
    pub recipient: TlsByteVecU32,
}

impl GroupMessage {
    /// Create a new `GroupMessage` taking an `MlsMessageIn` and slice of
    /// recipient names.
    pub fn new(msg: MlsMessageIn, recipient: Vec<u8>) -> Self {
        Self {
            msg,
            recipient: recipient.clone().into(),
        }
    }
}

impl tls_codec::Size for ClientInfo {
    fn tls_serialized_len(&self) -> usize {
        TlsByteSliceU16(self.client_name.as_bytes()).tls_serialized_len()
            + self.key_packages.tls_serialized_len()
    }
}

impl tls_codec::Serialize for ClientInfo {
    fn tls_serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<usize, tls_codec::Error> {
        let written = TlsByteSliceU16(self.client_name.as_bytes()).tls_serialize(writer)?;
        self.key_packages.tls_serialize(writer).map(|l| l + written)
    }
}

impl tls_codec::Deserialize for ClientInfo {
    fn tls_deserialize<R: std::io::Read>(bytes: &mut R) -> Result<Self, tls_codec::Error> {
        let client_name =
            String::from_utf8_lossy(TlsByteVecU16::tls_deserialize(bytes)?.as_slice()).into();

        let mut key_packages: Vec<(TlsByteVecU8, KeyPackageIn)> =
            TlsVecU32::<(TlsByteVecU8, KeyPackageIn)>::tls_deserialize(bytes)?.into();
        let key_packages = key_packages
            .drain(..)
            .map(|(e1, e2)| (e1.into(), e2))
            .collect();
        Ok(Self::new(client_name, key_packages))
    }
}

impl tls_codec::Size for RegisterClientParams {
    fn tls_serialized_len(&self) -> usize {
        self.userid.as_bytes().tls_serialized_len()
            + self.timestamp.tls_serialized_len()
            + self.key_packages.tls_serialized_len()
            + self.payload_hash.as_bytes().tls_serialized_len()
            + self
                .web3mq_user_mainkey_signature
                .as_bytes()
                .tls_serialized_len()
    }
}

impl tls_codec::Serialize for RegisterClientParams {
    fn tls_serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<usize, tls_codec::Error> {
        let written = self.userid.as_bytes().tls_serialize(writer)?;
        self.timestamp.tls_serialize(writer)?;
        self.key_packages.tls_serialize(writer)?;
        self.payload_hash.as_bytes().tls_serialize(writer)?;
        self.web3mq_user_mainkey_signature
            .as_bytes()
            .tls_serialize(writer)
            .map(|l| l + written)
    }
}

impl tls_codec::Deserialize for RegisterClientParams {
    fn tls_deserialize<R: std::io::Read>(bytes: &mut R) -> Result<Self, tls_codec::Error> {
        let user_id =
            String::from_utf8_lossy(TlsByteVecU16::tls_deserialize(bytes)?.as_slice()).into();
        let timestamp = u64::tls_deserialize(bytes)?;
        let key_packages = TlsVecU32::<(Vec<u8>, Vec<u8>)>::tls_deserialize(bytes)?;
        let payload_hash =
            String::from_utf8_lossy(TlsByteVecU16::tls_deserialize(bytes)?.as_slice()).into();
        let web3mq_user_mainkey_signature =
            String::from_utf8_lossy(TlsByteVecU16::tls_deserialize(bytes)?.as_slice()).into();
        Ok(Self {
            userid: user_id,
            timestamp: timestamp,
            key_packages: key_packages,
            payload_hash: payload_hash,
            web3mq_user_mainkey_signature: web3mq_user_mainkey_signature,
        })
    }
}

impl tls_codec::Size for GroupMessage {
    fn tls_serialized_len(&self) -> usize {
        self.msg.tls_serialized_len() + self.recipient.tls_serialized_len()
    }
}

impl tls_codec::Serialize for GroupMessage {
    fn tls_serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<usize, tls_codec::Error> {
        let written = self.msg.tls_serialize(writer)?;
        self.recipient.tls_serialize(writer).map(|l| l + written)
    }
}

impl tls_codec::Deserialize for GroupMessage {
    fn tls_deserialize<R: std::io::Read>(bytes: &mut R) -> Result<Self, tls_codec::Error> {
        let msg = MlsMessageIn::tls_deserialize(bytes)?;
        let recipient = TlsByteVecU32::tls_deserialize(bytes)?;
        Ok(Self { msg, recipient })
    }
}
