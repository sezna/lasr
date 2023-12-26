#![allow(unused)]
use std::{collections::BTreeMap, hash::Hash, fmt::{Debug, LowerHex}, ops::{AddAssign, SubAssign}};
use eigenda_client::batch::BatchHeaderHash;
use ethereum_types::U256;
use serde::{Serialize, Deserialize};
use secp256k1::PublicKey;
use sha3::{Digest, Sha3_256, Keccak256};
use crate::{certificate::{RecoverableSignature, Certificate}, RecoverableSignatureBuilder, AccountCacheError};

pub type AccountResult<T> = Result<T, Box<dyn std::error::Error + Send>>;
/// Represents a 20-byte Ethereum Compatible address.
/// 
/// This structure is used to store Ethereum Compatible addresses, which are 
/// derived from the public key. It implements traits like Clone, Copy, Debug,
/// Serialize, Deserialize, etc., for ease of use across various contexts.

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct Address([u8; 20]);

impl From<[u8; 20]> for Address {
    fn from(value: [u8; 20]) -> Self {
        Address(value)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl From<Address> for [u8; 20] {
    fn from(value: Address) -> Self {
        value.0
    }
}

impl LowerHex for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}
impl From<ethereum_types::H160> for Address {
    fn from(value: ethereum_types::H160) -> Self {
        Address::new(value.0)
    }
}

impl Address {
    fn new(bytes: [u8; 20]) -> Address {
        Address(bytes)
    }
}

/// Represents a 32-byte account hash.
///
/// This structure is used to store current state hash associated with an account
// It supports standard traits for easy handling and
/// comparison operations.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountHash([u8; 32]);

impl AccountHash {
    /// Creates a new `AccountHash` instance from a 32-byte array.
    ///
    /// This constructor is used to instantiate an `AccountHash` object with a given hash.
    pub fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }
}

impl From<PublicKey> for Address {
    /// Converts a `PublicKey` into an `Address`.
    ///
    /// This function takes a public key, serializes it, and then performs Keccak256
    /// hashing to derive the Ethereum address. It returns the last 20 bytes of the hash
    /// as the address.
    fn from(value: PublicKey) -> Self {
        let serialized_pk = value.serialize_uncompressed();

        let mut hasher = Keccak256::new();

        hasher.update(&serialized_pk[1..]);

        let result = hasher.finalize();
        let address_bytes = &result[result.len() - 20..];
        let mut address = [0u8; 20];
        address.copy_from_slice(address_bytes);

        Address(address)
    }
}

/// Represents an LASR account.
///
/// This structure contains details of an LASR account, including its address, associated
/// programs, nonce, signatures, hashes, and certificates. It implements traits for
/// serialization, hashing, and comparison.
#[derive(Builder, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct Account {
    address: Address,
    programs: BTreeMap<Address, Token>,
    nonce: U256,
}

impl Account {
    /// Constructs a new `Account` with the given address and optional program data.
    ///
    /// This function initializes an account with the provided address and an optional
    /// map of programs. It updates the account hash before returning.
    pub fn new(
        address: Address,
        programs: Option<BTreeMap<Address, Token>>
    ) -> Self {
        let mut account = Self {
            address,
            programs: BTreeMap::new(),
            nonce: U256::default(),
        };

        account
    }

    pub fn address(&self) -> Address {
        self.address.clone()
    }

    pub fn nonce(&self) -> U256 {
        self.nonce
    }

    pub fn programs(&self) -> &BTreeMap<Address, Token> {
        &self.programs
    }

    pub fn programs_mut(&mut self) -> &mut BTreeMap<Address, Token> {
        &mut self.programs
    }

    pub fn balance(&self, program_id: &Address) -> U256 {
        if let Some(entry) = self.programs().get(program_id) {
            return entry.balance()
        }

        return 0.into()
    }

    /// Updates the program data for a specific program address.
    ///
    /// This method either updates the existing program data or inserts new data if
    /// it doesn't exist for the given program address.
    pub(crate) fn update_programs(
        &mut self,
        program_id: &Address,
        delta: &TokenDelta
    ) {
        match self.programs.get_mut(&delta.token().program_id()) {
            Some(mut entry) => {
                entry.update_balance(*delta.receive(), *delta.send()).clone();
                log::info!("after updating, entry amount: {}", entry.balance());
            },
            None => { 
                self.programs.insert(program_id.clone(), delta.token.clone());
            }
        }
    }

    pub(crate) fn apply_send_transaction(&mut self, transaction: Transaction) -> AccountResult<()> {
        if !transaction.transaction_type().is_send() {
            return Err(Box::new(AccountCacheError) as Box<dyn std::error::Error + Send>)
        }

        Ok(())
    }

    pub(crate) fn insert_program(&mut self, program_id: &Address, token: Token) {
        self.programs.insert(program_id.clone(), token);
    }

    pub(crate) fn validate_program_id(&self, program_id: &Address) -> AccountResult<()> {
        if let Some(token) = self.programs.get(program_id) {
            return Ok(())
        }

        return Err(Box::new(AccountCacheError))
    }

    pub(crate) fn validate_balance(&self, program_id: &Address, amount: U256) -> AccountResult<()> {
        if let Some(token) = self.programs.get(program_id) {
            if token.balance() >= amount {
                return Ok(())
            }
        }

        return Err(Box::new(AccountCacheError))
    }
}

/// Represents a generic data container.
///
/// This structure is used to store arbitrary data as a vector of bytes (`Vec<u8>`).
/// It provides a default, cloneable, serializable, and debuggable interface. It is
/// typically used for storing data that doesn't have a fixed format or structure.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct ArbitraryData(Vec<u8>);

impl ArbitraryData {
    pub fn new() -> Self {
        Self(vec![])
    }
}

impl AsRef<[u8]> for ArbitraryData {
    /// Provides a reference to the internal byte array.
    ///
    /// This method enables the `ArbitraryData` struct to be easily converted into a
    /// byte slice reference, facilitating interoperability with functions expecting
    /// a byte slice.
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Represents metadata as a byte vector.
///
/// This structure is designed to encapsulate metadata, stored as a vector of bytes.
/// It supports cloning, serialization, and debugging. The metadata can be of any
/// form that fits into a byte array, making it a flexible container.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct Metadata(Vec<u8>);

impl Metadata {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Metadata {
    /// Provides a reference to the internal byte array.
    ///
    /// This implementation allows instances of `Metadata` to be passed to functions
    /// that require a reference to a byte slice, thereby facilitating easy access
    /// to the underlying data.
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Builder, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct Token {
    program_id: Address,
    owner_id: Address,
    balance: U256,
    metadata: Metadata,
    token_ids: Vec<U256>,
    allowance: BTreeMap<Address, U256>,
    approvals: BTreeMap<Address, U256>,
    data: ArbitraryData,
    status: Status,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct TokenDelta {
    token: Token,
    send: U256, 
    receive: U256,
}

impl TokenDelta {
    pub fn new(token: Token, send: U256, receive: U256) -> Self {
        Self { token, send, receive }
    }

    pub fn program_id(&self) -> Address {
        self.token().program_id()
    }

    pub fn token(&self) -> &Token {
        &self.token
    }
    
    pub fn send(&self) -> &U256 {
        &self.send
    }
    
    pub fn receive(&self) -> &U256 {
        &self.receive
    }
}

impl Token {
    pub fn program_id(&self) -> Address {
        self.program_id
    }

    pub fn update_balance(&mut self, receive: U256, send: U256) {
        self.balance += receive;
        self.balance -= send;
    }

    pub fn balance(&self) -> U256 {
        self.balance
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub enum Status {
    Locked,
    Free,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub enum TransactionType {
    BridgeIn(U256),
    Send(U256),
    Call(U256),
    BridgeOut(U256),
    Deploy(U256)
}

impl TransactionType {
    pub fn is_send(&self) -> bool {
        match self {
            TransactionType::Send(_) => true,
            _ => false
        }
    }

    pub fn is_bridge_in(&self) -> bool {
        match self {
            TransactionType::BridgeIn(_) => true,
            _ => false
        }
    }

    pub fn is_call(&self) -> bool {
        match self {
            TransactionType::Call(_) => true,
            _ => false
        }
    }
    
    pub fn is_bridge_out(&self) -> bool {
        match self {
            TransactionType::BridgeOut(_) => true,
            _ => false
        }
    }

    pub fn is_deploy(&self) -> bool {
        match self {
            TransactionType::Deploy(_) => true,
            _ => false
        }
    }
}

impl ToString for TransactionType {
    fn to_string(&self) -> String {
        match self {
            TransactionType::BridgeIn(n) => format!("bridgeIn{n}"),
            TransactionType::Send(n) => format!("send{n}").to_string(),
            TransactionType::Call(n) => format!("call{n}").to_string(),
            TransactionType::BridgeOut(n) => "bridgeOut".to_string(),
            TransactionType::Deploy(n) => "deploy".to_string()
        }
    }
}

#[derive(Builder, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct Payload {
    transaction_type: TransactionType,
    from: [u8; 20],
    to: [u8; 20],
    program_id: [u8; 20],
    inputs: String,
    value: U256,
}

impl Payload {
    fn transaction_type(&self) -> TransactionType {
        self.transaction_type.clone()
    }

    fn from(&self) -> [u8; 20] {
        self.from
    }

    fn to(&self) -> [u8; 20] {
        self.to
    }

    fn program_id(&self) -> [u8; 20] {
        self.program_id
    }

    fn inputs(&self) -> String {
        self.inputs.clone()
    }

    fn value(&self) -> U256 {
        self.value
    }

    pub fn hash_string(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.as_bytes());
        let res = hasher.finalize();
        format!("0x{:x}", res)
    }

    pub fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.as_bytes());
        let res = hasher.finalize();
        res.to_vec()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.transaction_type().to_string().as_bytes());
        bytes.extend_from_slice(&self.from().as_ref());
        bytes.extend_from_slice(&self.to().as_ref());
        bytes.extend_from_slice(&self.program_id().as_ref());
        bytes.extend_from_slice(self.inputs().to_string().as_bytes());
        let mut u256 = Vec::new(); 
        let value = self.value();
        value.0.iter().for_each(|n| { 
            let le = n.to_le_bytes();
            u256.extend_from_slice(&le);
        }); 
        bytes.extend_from_slice(&u256);
        bytes
    }
}

#[derive(Builder, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)] 
pub struct Transaction {
    transaction_type: TransactionType,
    from: [u8; 20],
    to: [u8; 20],
    program_id: [u8; 20],
    inputs: String,
    value: U256,
    v: i32,
    r: [u8; 32],
    s: [u8; 32],
}

impl Transaction {
    pub fn program_id(&self) -> Address {
        self.program_id.into()
    }

    pub fn from(&self) -> Address {
        self.from.into()
    }

    pub fn to(&self) -> Address {
        self.to.into()
    }

    pub fn transaction_type(&self) -> TransactionType {
        self.transaction_type.clone()
    }

    pub fn inputs(&self) -> String {
        self.inputs.to_string()
    }

    pub fn value(&self) -> U256 {
        self.value
    }

    pub fn sig(&self) -> Result<RecoverableSignature, Box<dyn std::error::Error>> { 
        let sig = RecoverableSignatureBuilder::default()
            .r(self.r)
            .s(self.s)
            .v(self.v)
            .build().map_err(|e| Box::new(e))?;

        Ok(sig)
    }

    pub fn recover(&self) -> Result<PublicKey, Box<dyn std::error::Error>> {
        let pk = self.sig()?.recover(&self.as_bytes())?;
        Ok(pk)
    }

    pub fn message(&self) -> String {
        format!("{:02x}", self)
    }

    pub fn hash_string(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.as_bytes());
        let res = hasher.finalize();
        format!("0x{:x}", res)
    }

    pub fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.as_bytes());
        let res = hasher.finalize();
        res.to_vec()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.transaction_type().to_string().as_bytes());
        bytes.extend_from_slice(&self.from().as_ref());
        bytes.extend_from_slice(&self.to().as_ref());
        bytes.extend_from_slice(&self.program_id().as_ref());
        bytes.extend_from_slice(self.inputs().to_string().as_bytes());
        let mut u256 = Vec::new(); 
        let value = self.value();
        value.0.iter().for_each(|n| { 
            let le = n.to_le_bytes();
            u256.extend_from_slice(&le);
        }); 
        bytes.extend_from_slice(&u256);
        bytes
    }

    pub fn verify_signature(&self) -> Result<(), secp256k1::Error> {
        self.sig().map_err(|e| secp256k1::Error::InvalidMessage)?.verify(&self.as_bytes())
    }
}

impl AddAssign for Token {
    fn add_assign(&mut self, rhs: Self) {
        self.balance += rhs.balance();
    }
}

impl SubAssign for Token {
    fn sub_assign(&mut self, rhs: Self) {
        self.balance -= rhs.balance();
    }
}

impl LowerHex for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.as_bytes() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Transaction {
            transaction_type: TransactionType::BridgeIn(0.into()),
            from: [0; 20],
            to: [0; 20],
            program_id: [0; 20],
            inputs: String::new(),
            value: 0.into(),
            v: 0,
            r: [0; 32],
            s: [0; 32]
        }
    }
}

impl From<(Payload, RecoverableSignature)> for Transaction {
    fn from(value: (Payload, RecoverableSignature)) -> Self {
        Transaction { 
            transaction_type: value.0.transaction_type(), 
            from: value.0.from(), 
            to: value.0.to(), 
            program_id: value.0.program_id(), 
            inputs: value.0.inputs(), 
            value: value.0.value(), 
            v: value.1.get_v(), 
            r: value.1.get_r(), 
            s: value.1.get_s() 
        }
    }
}
