//! Mock types.

use crate::{
    identifier::{SenderIdentifiers, TransactionId},
    pool::txpool::TxPool,
    traits::TransactionOrigin,
    CoinbaseTipOrdering, EthBlobTransactionSidecar, EthPoolTransaction, PoolTransaction,
    ValidPoolTransaction,
};
use alloy_consensus::{
    constants::{
        EIP1559_TX_TYPE_ID, EIP2930_TX_TYPE_ID, EIP4844_TX_TYPE_ID, EIP7702_TX_TYPE_ID,
        LEGACY_TX_TYPE_ID,
    },
    EthereumTxEnvelope, Signed, TxEip1559, TxEip2930, TxEip4844, TxEip4844Variant, TxEip7702,
    TxLegacy, TxType, Typed2718,
};
use alloy_eips::{
    eip1559::MIN_PROTOCOL_BASE_FEE,
    eip2930::AccessList,
    eip4844::{BlobTransactionSidecar, BlobTransactionValidationError, DATA_GAS_PER_BLOB},
    eip7594::BlobTransactionSidecarVariant,
    eip7702::SignedAuthorization,
};
use alloy_primitives::{Address, Bytes, ChainId, Signature, TxHash, TxKind, B256, U256};
use paste::paste;
use rand::{distr::Uniform, prelude::Distribution};
use reth_ethereum_primitives::{PooledTransactionVariant, Transaction, TransactionSigned};
use reth_primitives_traits::{
    transaction::error::TryFromRecoveredTransactionError, InMemorySize, Recovered,
    SignedTransaction,
};

use alloy_consensus::error::ValueError;
use alloy_eips::eip4844::env_settings::KzgSettings;
use rand::distr::weighted::WeightedIndex;
use std::{ops::Range, sync::Arc, time::Instant, vec::IntoIter};

/// A transaction pool implementation using [`MockOrdering`] for transaction ordering.
///
/// This type is an alias for [`TxPool<MockOrdering>`].
pub type MockTxPool = TxPool<MockOrdering>;

/// A validated transaction in the transaction pool, using [`MockTransaction`] as the transaction
/// type.
///
/// This type is an alias for [`ValidPoolTransaction<MockTransaction>`].
pub type MockValidTx = ValidPoolTransaction<MockTransaction>;

/// Create an empty `TxPool`
pub fn mock_tx_pool() -> MockTxPool {
    MockTxPool::new(Default::default(), Default::default())
}

/// Sets the value for the field
macro_rules! set_value {
    ($this:ident => $field:ident) => {
        let new_value = $field;
        match $this {
            MockTransaction::Legacy { ref mut $field, .. } |
            MockTransaction::Eip1559 { ref mut $field, .. } |
            MockTransaction::Eip4844 { ref mut $field, .. } |
            MockTransaction::Eip2930 { ref mut $field, .. } |
            MockTransaction::Eip7702 { ref mut $field, .. } => {
                *$field = new_value;
            }
        }
        // Ensure the tx cost is always correct after each mutation.
        $this.update_cost();
    };
}

/// Gets the value for the field
macro_rules! get_value {
    ($this:tt => $field:ident) => {
        match $this {
            MockTransaction::Legacy { $field, .. } |
            MockTransaction::Eip1559 { $field, .. } |
            MockTransaction::Eip4844 { $field, .. } |
            MockTransaction::Eip2930 { $field, .. } |
            MockTransaction::Eip7702 { $field, .. } => $field,
        }
    };
}

// Generates all setters and getters
macro_rules! make_setters_getters {
    ($($name:ident => $t:ty);*) => {
        paste! {$(
            /// Sets the value of the specified field.
            pub fn [<set_ $name>](&mut self, $name: $t) -> &mut Self {
                set_value!(self => $name);
                self
            }

            /// Sets the value of the specified field using a fluent interface.
            pub fn [<with_ $name>](mut self, $name: $t) -> Self {
                set_value!(self => $name);
                self
            }

            /// Gets the value of the specified field.
            pub const fn [<get_ $name>](&self) -> &$t {
                get_value!(self => $name)
            }
        )*}
    };
}

/// A Bare transaction type used for testing.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MockTransaction {
    /// Legacy transaction type.
    Legacy {
        /// The chain id of the transaction.
        chain_id: Option<ChainId>,
        /// The hash of the transaction.
        hash: B256,
        /// The sender's address.
        sender: Address,
        /// The transaction nonce.
        nonce: u64,
        /// The gas price for the transaction.
        gas_price: u128,
        /// The gas limit for the transaction.
        gas_limit: u64,
        /// The transaction's destination.
        to: TxKind,
        /// The value of the transaction.
        value: U256,
        /// The transaction input data.
        input: Bytes,
        /// The size of the transaction, returned in the implementation of [`PoolTransaction`].
        size: usize,
        /// The cost of the transaction, returned in the implementation of [`PoolTransaction`].
        cost: U256,
    },
    /// EIP-2930 transaction type.
    Eip2930 {
        /// The chain id of the transaction.
        chain_id: ChainId,
        /// The hash of the transaction.
        hash: B256,
        /// The sender's address.
        sender: Address,
        /// The transaction nonce.
        nonce: u64,
        /// The transaction's destination.
        to: TxKind,
        /// The gas limit for the transaction.
        gas_limit: u64,
        /// The transaction input data.
        input: Bytes,
        /// The value of the transaction.
        value: U256,
        /// The gas price for the transaction.
        gas_price: u128,
        /// The access list associated with the transaction.
        access_list: AccessList,
        /// The size of the transaction, returned in the implementation of [`PoolTransaction`].
        size: usize,
        /// The cost of the transaction, returned in the implementation of [`PoolTransaction`].
        cost: U256,
    },
    /// EIP-1559 transaction type.
    Eip1559 {
        /// The chain id of the transaction.
        chain_id: ChainId,
        /// The hash of the transaction.
        hash: B256,
        /// The sender's address.
        sender: Address,
        /// The transaction nonce.
        nonce: u64,
        /// The maximum fee per gas for the transaction.
        max_fee_per_gas: u128,
        /// The maximum priority fee per gas for the transaction.
        max_priority_fee_per_gas: u128,
        /// The gas limit for the transaction.
        gas_limit: u64,
        /// The transaction's destination.
        to: TxKind,
        /// The value of the transaction.
        value: U256,
        /// The access list associated with the transaction.
        access_list: AccessList,
        /// The transaction input data.
        input: Bytes,
        /// The size of the transaction, returned in the implementation of [`PoolTransaction`].
        size: usize,
        /// The cost of the transaction, returned in the implementation of [`PoolTransaction`].
        cost: U256,
    },
    /// EIP-4844 transaction type.
    Eip4844 {
        /// The chain id of the transaction.
        chain_id: ChainId,
        /// The hash of the transaction.
        hash: B256,
        /// The sender's address.
        sender: Address,
        /// The transaction nonce.
        nonce: u64,
        /// The maximum fee per gas for the transaction.
        max_fee_per_gas: u128,
        /// The maximum priority fee per gas for the transaction.
        max_priority_fee_per_gas: u128,
        /// The maximum fee per blob gas for the transaction.
        max_fee_per_blob_gas: u128,
        /// The gas limit for the transaction.
        gas_limit: u64,
        /// The transaction's destination.
        to: Address,
        /// The value of the transaction.
        value: U256,
        /// The access list associated with the transaction.
        access_list: AccessList,
        /// The transaction input data.
        input: Bytes,
        /// The sidecar information for the transaction.
        sidecar: BlobTransactionSidecarVariant,
        /// The blob versioned hashes for the transaction.
        blob_versioned_hashes: Vec<B256>,
        /// The size of the transaction, returned in the implementation of [`PoolTransaction`].
        size: usize,
        /// The cost of the transaction, returned in the implementation of [`PoolTransaction`].
        cost: U256,
    },
    /// EIP-7702 transaction type.
    Eip7702 {
        /// The chain id of the transaction.
        chain_id: ChainId,
        /// The hash of the transaction.
        hash: B256,
        /// The sender's address.
        sender: Address,
        /// The transaction nonce.
        nonce: u64,
        /// The maximum fee per gas for the transaction.
        max_fee_per_gas: u128,
        /// The maximum priority fee per gas for the transaction.
        max_priority_fee_per_gas: u128,
        /// The gas limit for the transaction.
        gas_limit: u64,
        /// The transaction's destination.
        to: Address,
        /// The value of the transaction.
        value: U256,
        /// The access list associated with the transaction.
        access_list: AccessList,
        /// The authorization list associated with the transaction.
        authorization_list: Vec<SignedAuthorization>,
        /// The transaction input data.
        input: Bytes,
        /// The size of the transaction, returned in the implementation of [`PoolTransaction`].
        size: usize,
        /// The cost of the transaction, returned in the implementation of [`PoolTransaction`].
        cost: U256,
    },
}

// === impl MockTransaction ===

impl MockTransaction {
    make_setters_getters! {
        nonce => u64;
        hash => B256;
        sender => Address;
        gas_limit => u64;
        value => U256;
        input => Bytes;
        size => usize
    }

    /// Returns a new legacy transaction with random address and hash and empty values
    pub fn legacy() -> Self {
        Self::Legacy {
            chain_id: Some(1),
            hash: B256::random(),
            sender: Address::random(),
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,
            to: Address::random().into(),
            value: Default::default(),
            input: Default::default(),
            size: Default::default(),
            cost: U256::ZERO,
        }
    }

    /// Returns a new EIP2930 transaction with random address and hash and empty values
    pub fn eip2930() -> Self {
        Self::Eip2930 {
            chain_id: 1,
            hash: B256::random(),
            sender: Address::random(),
            nonce: 0,
            to: Address::random().into(),
            gas_limit: 0,
            input: Bytes::new(),
            value: Default::default(),
            gas_price: 0,
            access_list: Default::default(),
            size: Default::default(),
            cost: U256::ZERO,
        }
    }

    /// Returns a new EIP1559 transaction with random address and hash and empty values
    pub fn eip1559() -> Self {
        Self::Eip1559 {
            chain_id: 1,
            hash: B256::random(),
            sender: Address::random(),
            nonce: 0,
            max_fee_per_gas: MIN_PROTOCOL_BASE_FEE as u128,
            max_priority_fee_per_gas: MIN_PROTOCOL_BASE_FEE as u128,
            gas_limit: 0,
            to: Address::random().into(),
            value: Default::default(),
            input: Bytes::new(),
            access_list: Default::default(),
            size: Default::default(),
            cost: U256::ZERO,
        }
    }

    /// Returns a new EIP7702 transaction with random address and hash and empty values
    pub fn eip7702() -> Self {
        Self::Eip7702 {
            chain_id: 1,
            hash: B256::random(),
            sender: Address::random(),
            nonce: 0,
            max_fee_per_gas: MIN_PROTOCOL_BASE_FEE as u128,
            max_priority_fee_per_gas: MIN_PROTOCOL_BASE_FEE as u128,
            gas_limit: 0,
            to: Address::random(),
            value: Default::default(),
            input: Bytes::new(),
            access_list: Default::default(),
            authorization_list: vec![],
            size: Default::default(),
            cost: U256::ZERO,
        }
    }

    /// Returns a new EIP4844 transaction with random address and hash and empty values
    pub fn eip4844() -> Self {
        Self::Eip4844 {
            chain_id: 1,
            hash: B256::random(),
            sender: Address::random(),
            nonce: 0,
            max_fee_per_gas: MIN_PROTOCOL_BASE_FEE as u128,
            max_priority_fee_per_gas: MIN_PROTOCOL_BASE_FEE as u128,
            max_fee_per_blob_gas: DATA_GAS_PER_BLOB as u128,
            gas_limit: 0,
            to: Address::random(),
            value: Default::default(),
            input: Bytes::new(),
            access_list: Default::default(),
            sidecar: BlobTransactionSidecarVariant::Eip4844(Default::default()),
            blob_versioned_hashes: Default::default(),
            size: Default::default(),
            cost: U256::ZERO,
        }
    }

    /// Returns a new EIP4844 transaction with a provided sidecar
    pub fn eip4844_with_sidecar(sidecar: BlobTransactionSidecarVariant) -> Self {
        let mut transaction = Self::eip4844();
        if let Self::Eip4844 { sidecar: existing_sidecar, blob_versioned_hashes, .. } =
            &mut transaction
        {
            *blob_versioned_hashes = sidecar.versioned_hashes().collect();
            *existing_sidecar = sidecar;
        }
        transaction
    }

    /// Creates a new transaction with the given [`TxType`].
    ///
    /// See the default constructors for each of the transaction types:
    ///
    /// * [`MockTransaction::legacy`]
    /// * [`MockTransaction::eip2930`]
    /// * [`MockTransaction::eip1559`]
    /// * [`MockTransaction::eip4844`]
    pub fn new_from_type(tx_type: TxType) -> Self {
        match tx_type {
            TxType::Legacy => Self::legacy(),
            TxType::Eip2930 => Self::eip2930(),
            TxType::Eip1559 => Self::eip1559(),
            TxType::Eip4844 => Self::eip4844(),
            TxType::Eip7702 => Self::eip7702(),
        }
    }

    /// Sets the max fee per blob gas for EIP-4844 transactions,
    pub const fn with_blob_fee(mut self, val: u128) -> Self {
        self.set_blob_fee(val);
        self
    }

    /// Sets the max fee per blob gas for EIP-4844 transactions,
    pub const fn set_blob_fee(&mut self, val: u128) -> &mut Self {
        if let Self::Eip4844 { max_fee_per_blob_gas, .. } = self {
            *max_fee_per_blob_gas = val;
        }
        self
    }

    /// Sets the priority fee for dynamic fee transactions (EIP-1559 and EIP-4844)
    pub const fn set_priority_fee(&mut self, val: u128) -> &mut Self {
        if let Self::Eip1559 { max_priority_fee_per_gas, .. } |
        Self::Eip4844 { max_priority_fee_per_gas, .. } = self
        {
            *max_priority_fee_per_gas = val;
        }
        self
    }

    /// Sets the priority fee for dynamic fee transactions (EIP-1559 and EIP-4844)
    pub const fn with_priority_fee(mut self, val: u128) -> Self {
        self.set_priority_fee(val);
        self
    }

    /// Gets the priority fee for dynamic fee transactions (EIP-1559 and EIP-4844)
    pub const fn get_priority_fee(&self) -> Option<u128> {
        match self {
            Self::Eip1559 { max_priority_fee_per_gas, .. } |
            Self::Eip4844 { max_priority_fee_per_gas, .. } |
            Self::Eip7702 { max_priority_fee_per_gas, .. } => Some(*max_priority_fee_per_gas),
            _ => None,
        }
    }

    /// Sets the max fee for dynamic fee transactions (EIP-1559 and EIP-4844)
    pub const fn set_max_fee(&mut self, val: u128) -> &mut Self {
        if let Self::Eip1559 { max_fee_per_gas, .. } |
        Self::Eip4844 { max_fee_per_gas, .. } |
        Self::Eip7702 { max_fee_per_gas, .. } = self
        {
            *max_fee_per_gas = val;
        }
        self
    }

    /// Sets the max fee for dynamic fee transactions (EIP-1559 and EIP-4844)
    pub const fn with_max_fee(mut self, val: u128) -> Self {
        self.set_max_fee(val);
        self
    }

    /// Gets the max fee for dynamic fee transactions (EIP-1559 and EIP-4844)
    pub const fn get_max_fee(&self) -> Option<u128> {
        match self {
            Self::Eip1559 { max_fee_per_gas, .. } |
            Self::Eip4844 { max_fee_per_gas, .. } |
            Self::Eip7702 { max_fee_per_gas, .. } => Some(*max_fee_per_gas),
            _ => None,
        }
    }

    /// Sets the access list for transactions supporting EIP-1559, EIP-4844, and EIP-2930.
    pub fn set_accesslist(&mut self, list: AccessList) -> &mut Self {
        match self {
            Self::Legacy { .. } => {}
            Self::Eip1559 { access_list: accesslist, .. } |
            Self::Eip4844 { access_list: accesslist, .. } |
            Self::Eip2930 { access_list: accesslist, .. } |
            Self::Eip7702 { access_list: accesslist, .. } => {
                *accesslist = list;
            }
        }
        self
    }

    /// Sets the authorization list for EIP-7702 transactions.
    pub fn set_authorization_list(&mut self, list: Vec<SignedAuthorization>) -> &mut Self {
        if let Self::Eip7702 { authorization_list, .. } = self {
            *authorization_list = list;
        }

        self
    }

    /// Sets the gas price for the transaction.
    pub const fn set_gas_price(&mut self, val: u128) -> &mut Self {
        match self {
            Self::Legacy { gas_price, .. } | Self::Eip2930 { gas_price, .. } => {
                *gas_price = val;
            }
            Self::Eip1559 { max_fee_per_gas, max_priority_fee_per_gas, .. } |
            Self::Eip4844 { max_fee_per_gas, max_priority_fee_per_gas, .. } |
            Self::Eip7702 { max_fee_per_gas, max_priority_fee_per_gas, .. } => {
                *max_fee_per_gas = val;
                *max_priority_fee_per_gas = val;
            }
        }
        self
    }

    /// Sets the gas price for the transaction.
    pub const fn with_gas_price(mut self, val: u128) -> Self {
        match self {
            Self::Legacy { ref mut gas_price, .. } | Self::Eip2930 { ref mut gas_price, .. } => {
                *gas_price = val;
            }
            Self::Eip1559 { ref mut max_fee_per_gas, ref mut max_priority_fee_per_gas, .. } |
            Self::Eip4844 { ref mut max_fee_per_gas, ref mut max_priority_fee_per_gas, .. } |
            Self::Eip7702 { ref mut max_fee_per_gas, ref mut max_priority_fee_per_gas, .. } => {
                *max_fee_per_gas = val;
                *max_priority_fee_per_gas = val;
            }
        }
        self
    }

    /// Gets the gas price for the transaction.
    pub const fn get_gas_price(&self) -> u128 {
        match self {
            Self::Legacy { gas_price, .. } | Self::Eip2930 { gas_price, .. } => *gas_price,
            Self::Eip1559 { max_fee_per_gas, .. } |
            Self::Eip4844 { max_fee_per_gas, .. } |
            Self::Eip7702 { max_fee_per_gas, .. } => *max_fee_per_gas,
        }
    }

    /// Returns a clone with a decreased nonce
    pub fn prev(&self) -> Self {
        self.clone().with_hash(B256::random()).with_nonce(self.get_nonce() - 1)
    }

    /// Returns a clone with an increased nonce
    pub fn next(&self) -> Self {
        self.clone().with_hash(B256::random()).with_nonce(self.get_nonce() + 1)
    }

    /// Returns a clone with an increased nonce
    pub fn skip(&self, skip: u64) -> Self {
        self.clone().with_hash(B256::random()).with_nonce(self.get_nonce() + skip + 1)
    }

    /// Returns a clone with incremented nonce
    pub fn inc_nonce(self) -> Self {
        let nonce = self.get_nonce() + 1;
        self.with_nonce(nonce)
    }

    /// Sets a new random hash
    pub fn rng_hash(self) -> Self {
        self.with_hash(B256::random())
    }

    /// Returns a new transaction with a higher gas price +1
    pub fn inc_price(&self) -> Self {
        self.inc_price_by(1)
    }

    /// Returns a new transaction with a higher gas price
    pub fn inc_price_by(&self, value: u128) -> Self {
        self.clone().with_gas_price(self.get_gas_price().checked_add(value).unwrap())
    }

    /// Returns a new transaction with a lower gas price -1
    pub fn decr_price(&self) -> Self {
        self.decr_price_by(1)
    }

    /// Returns a new transaction with a lower gas price
    pub fn decr_price_by(&self, value: u128) -> Self {
        self.clone().with_gas_price(self.get_gas_price().checked_sub(value).unwrap())
    }

    /// Returns a new transaction with a higher value
    pub fn inc_value(&self) -> Self {
        self.clone().with_value(self.get_value().checked_add(U256::from(1)).unwrap())
    }

    /// Returns a new transaction with a higher gas limit
    pub fn inc_limit(&self) -> Self {
        self.clone().with_gas_limit(self.get_gas_limit() + 1)
    }

    /// Returns a new transaction with a higher blob fee +1
    ///
    /// If it's an EIP-4844 transaction.
    pub fn inc_blob_fee(&self) -> Self {
        self.inc_blob_fee_by(1)
    }

    /// Returns a new transaction with a higher blob fee
    ///
    /// If it's an EIP-4844 transaction.
    pub fn inc_blob_fee_by(&self, value: u128) -> Self {
        let mut this = self.clone();
        if let Self::Eip4844 { max_fee_per_blob_gas, .. } = &mut this {
            *max_fee_per_blob_gas = max_fee_per_blob_gas.checked_add(value).unwrap();
        }
        this
    }

    /// Returns a new transaction with a lower blob fee -1
    ///
    /// If it's an EIP-4844 transaction.
    pub fn decr_blob_fee(&self) -> Self {
        self.decr_price_by(1)
    }

    /// Returns a new transaction with a lower blob fee
    ///
    /// If it's an EIP-4844 transaction.
    pub fn decr_blob_fee_by(&self, value: u128) -> Self {
        let mut this = self.clone();
        if let Self::Eip4844 { max_fee_per_blob_gas, .. } = &mut this {
            *max_fee_per_blob_gas = max_fee_per_blob_gas.checked_sub(value).unwrap();
        }
        this
    }

    /// Returns the transaction type identifier associated with the current [`MockTransaction`].
    pub const fn tx_type(&self) -> u8 {
        match self {
            Self::Legacy { .. } => LEGACY_TX_TYPE_ID,
            Self::Eip1559 { .. } => EIP1559_TX_TYPE_ID,
            Self::Eip4844 { .. } => EIP4844_TX_TYPE_ID,
            Self::Eip2930 { .. } => EIP2930_TX_TYPE_ID,
            Self::Eip7702 { .. } => EIP7702_TX_TYPE_ID,
        }
    }

    /// Checks if the transaction is of the legacy type.
    pub const fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy { .. })
    }

    /// Checks if the transaction is of the EIP-1559 type.
    pub const fn is_eip1559(&self) -> bool {
        matches!(self, Self::Eip1559 { .. })
    }

    /// Checks if the transaction is of the EIP-4844 type.
    pub const fn is_eip4844(&self) -> bool {
        matches!(self, Self::Eip4844 { .. })
    }

    /// Checks if the transaction is of the EIP-2930 type.
    pub const fn is_eip2930(&self) -> bool {
        matches!(self, Self::Eip2930 { .. })
    }

    /// Checks if the transaction is of the EIP-7702 type.
    pub const fn is_eip7702(&self) -> bool {
        matches!(self, Self::Eip7702 { .. })
    }

    fn update_cost(&mut self) {
        match self {
            Self::Legacy { cost, gas_limit, gas_price, value, .. } |
            Self::Eip2930 { cost, gas_limit, gas_price, value, .. } => {
                *cost = U256::from(*gas_limit) * U256::from(*gas_price) + *value
            }
            Self::Eip1559 { cost, gas_limit, max_fee_per_gas, value, .. } |
            Self::Eip4844 { cost, gas_limit, max_fee_per_gas, value, .. } |
            Self::Eip7702 { cost, gas_limit, max_fee_per_gas, value, .. } => {
                *cost = U256::from(*gas_limit) * U256::from(*max_fee_per_gas) + *value
            }
        };
    }
}

impl PoolTransaction for MockTransaction {
    type TryFromConsensusError = ValueError<EthereumTxEnvelope<TxEip4844>>;

    type Consensus = TransactionSigned;

    type Pooled = PooledTransactionVariant;

    fn into_consensus(self) -> Recovered<Self::Consensus> {
        self.into()
    }

    fn from_pooled(pooled: Recovered<Self::Pooled>) -> Self {
        pooled.into()
    }

    fn hash(&self) -> &TxHash {
        self.get_hash()
    }

    fn sender(&self) -> Address {
        *self.get_sender()
    }

    fn sender_ref(&self) -> &Address {
        self.get_sender()
    }

    // Having `get_cost` from `make_setters_getters` would be cleaner but we didn't
    // want to also generate the error-prone cost setters. For now cost should be
    // correct at construction and auto-updated per field update via `update_cost`,
    // not to be manually set.
    fn cost(&self) -> &U256 {
        match self {
            Self::Legacy { cost, .. } |
            Self::Eip2930 { cost, .. } |
            Self::Eip1559 { cost, .. } |
            Self::Eip4844 { cost, .. } |
            Self::Eip7702 { cost, .. } => cost,
        }
    }

    /// Returns the encoded length of the transaction.
    fn encoded_length(&self) -> usize {
        self.size()
    }
}

impl InMemorySize for MockTransaction {
    fn size(&self) -> usize {
        *self.get_size()
    }
}

impl Typed2718 for MockTransaction {
    fn ty(&self) -> u8 {
        match self {
            Self::Legacy { .. } => TxType::Legacy.into(),
            Self::Eip1559 { .. } => TxType::Eip1559.into(),
            Self::Eip4844 { .. } => TxType::Eip4844.into(),
            Self::Eip2930 { .. } => TxType::Eip2930.into(),
            Self::Eip7702 { .. } => TxType::Eip7702.into(),
        }
    }
}

impl alloy_consensus::Transaction for MockTransaction {
    fn chain_id(&self) -> Option<u64> {
        match self {
            Self::Legacy { chain_id, .. } => *chain_id,
            Self::Eip1559 { chain_id, .. } |
            Self::Eip4844 { chain_id, .. } |
            Self::Eip2930 { chain_id, .. } |
            Self::Eip7702 { chain_id, .. } => Some(*chain_id),
        }
    }

    fn nonce(&self) -> u64 {
        *self.get_nonce()
    }

    fn gas_limit(&self) -> u64 {
        *self.get_gas_limit()
    }

    fn gas_price(&self) -> Option<u128> {
        match self {
            Self::Legacy { gas_price, .. } | Self::Eip2930 { gas_price, .. } => Some(*gas_price),
            _ => None,
        }
    }

    fn max_fee_per_gas(&self) -> u128 {
        match self {
            Self::Legacy { gas_price, .. } | Self::Eip2930 { gas_price, .. } => *gas_price,
            Self::Eip1559 { max_fee_per_gas, .. } |
            Self::Eip4844 { max_fee_per_gas, .. } |
            Self::Eip7702 { max_fee_per_gas, .. } => *max_fee_per_gas,
        }
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        match self {
            Self::Legacy { .. } | Self::Eip2930 { .. } => None,
            Self::Eip1559 { max_priority_fee_per_gas, .. } |
            Self::Eip4844 { max_priority_fee_per_gas, .. } |
            Self::Eip7702 { max_priority_fee_per_gas, .. } => Some(*max_priority_fee_per_gas),
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        match self {
            Self::Eip4844 { max_fee_per_blob_gas, .. } => Some(*max_fee_per_blob_gas),
            _ => None,
        }
    }

    fn priority_fee_or_price(&self) -> u128 {
        match self {
            Self::Legacy { gas_price, .. } | Self::Eip2930 { gas_price, .. } => *gas_price,
            Self::Eip1559 { max_priority_fee_per_gas, .. } |
            Self::Eip4844 { max_priority_fee_per_gas, .. } |
            Self::Eip7702 { max_priority_fee_per_gas, .. } => *max_priority_fee_per_gas,
        }
    }

    fn effective_gas_price(&self, base_fee: Option<u64>) -> u128 {
        base_fee.map_or_else(
            || self.max_fee_per_gas(),
            |base_fee| {
                // if the tip is greater than the max priority fee per gas, set it to the max
                // priority fee per gas + base fee
                let tip = self.max_fee_per_gas().saturating_sub(base_fee as u128);
                if let Some(max_tip) = self.max_priority_fee_per_gas() {
                    if tip > max_tip {
                        max_tip + base_fee as u128
                    } else {
                        // otherwise return the max fee per gas
                        self.max_fee_per_gas()
                    }
                } else {
                    self.max_fee_per_gas()
                }
            },
        )
    }

    fn is_dynamic_fee(&self) -> bool {
        !matches!(self, Self::Legacy { .. } | Self::Eip2930 { .. })
    }

    fn kind(&self) -> TxKind {
        match self {
            Self::Legacy { to, .. } | Self::Eip1559 { to, .. } | Self::Eip2930 { to, .. } => *to,
            Self::Eip4844 { to, .. } | Self::Eip7702 { to, .. } => TxKind::Call(*to),
        }
    }

    fn is_create(&self) -> bool {
        match self {
            Self::Legacy { to, .. } | Self::Eip1559 { to, .. } | Self::Eip2930 { to, .. } => {
                to.is_create()
            }
            Self::Eip4844 { .. } | Self::Eip7702 { .. } => false,
        }
    }

    fn value(&self) -> U256 {
        match self {
            Self::Legacy { value, .. } |
            Self::Eip1559 { value, .. } |
            Self::Eip2930 { value, .. } |
            Self::Eip4844 { value, .. } |
            Self::Eip7702 { value, .. } => *value,
        }
    }

    fn input(&self) -> &Bytes {
        self.get_input()
    }

    fn access_list(&self) -> Option<&AccessList> {
        match self {
            Self::Legacy { .. } => None,
            Self::Eip1559 { access_list: accesslist, .. } |
            Self::Eip4844 { access_list: accesslist, .. } |
            Self::Eip2930 { access_list: accesslist, .. } |
            Self::Eip7702 { access_list: accesslist, .. } => Some(accesslist),
        }
    }

    fn blob_versioned_hashes(&self) -> Option<&[B256]> {
        match self {
            Self::Eip4844 { blob_versioned_hashes, .. } => Some(blob_versioned_hashes),
            _ => None,
        }
    }

    fn authorization_list(&self) -> Option<&[SignedAuthorization]> {
        match self {
            Self::Eip7702 { authorization_list, .. } => Some(authorization_list),
            _ => None,
        }
    }
}

impl EthPoolTransaction for MockTransaction {
    fn take_blob(&mut self) -> EthBlobTransactionSidecar {
        match self {
            Self::Eip4844 { sidecar, .. } => EthBlobTransactionSidecar::Present(sidecar.clone()),
            _ => EthBlobTransactionSidecar::None,
        }
    }

    fn try_into_pooled_eip4844(
        self,
        sidecar: Arc<BlobTransactionSidecarVariant>,
    ) -> Option<Recovered<Self::Pooled>> {
        let (tx, signer) = self.into_consensus().into_parts();
        tx.try_into_pooled_eip4844(Arc::unwrap_or_clone(sidecar))
            .map(|tx| tx.with_signer(signer))
            .ok()
    }

    fn try_from_eip4844(
        tx: Recovered<Self::Consensus>,
        sidecar: BlobTransactionSidecarVariant,
    ) -> Option<Self> {
        let (tx, signer) = tx.into_parts();
        tx.try_into_pooled_eip4844(sidecar)
            .map(|tx| tx.with_signer(signer))
            .ok()
            .map(Self::from_pooled)
    }

    fn validate_blob(
        &self,
        _blob: &BlobTransactionSidecarVariant,
        _settings: &KzgSettings,
    ) -> Result<(), alloy_eips::eip4844::BlobTransactionValidationError> {
        match &self {
            Self::Eip4844 { .. } => Ok(()),
            _ => Err(BlobTransactionValidationError::NotBlobTransaction(self.tx_type())),
        }
    }
}

impl TryFrom<Recovered<TransactionSigned>> for MockTransaction {
    type Error = TryFromRecoveredTransactionError;

    fn try_from(tx: Recovered<TransactionSigned>) -> Result<Self, Self::Error> {
        let sender = tx.signer();
        let transaction = tx.into_inner();
        let hash = *transaction.tx_hash();
        let size = transaction.size();

        match transaction.into_typed_transaction() {
            Transaction::Legacy(TxLegacy {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                input,
            }) => Ok(Self::Legacy {
                chain_id,
                hash,
                sender,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                input,
                size,
                cost: U256::from(gas_limit) * U256::from(gas_price) + value,
            }),
            Transaction::Eip2930(TxEip2930 {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                input,
                access_list,
            }) => Ok(Self::Eip2930 {
                chain_id,
                hash,
                sender,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                input,
                access_list,
                size,
                cost: U256::from(gas_limit) * U256::from(gas_price) + value,
            }),
            Transaction::Eip1559(TxEip1559 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                input,
                access_list,
            }) => Ok(Self::Eip1559 {
                chain_id,
                hash,
                sender,
                nonce,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                gas_limit,
                to,
                value,
                input,
                access_list,
                size,
                cost: U256::from(gas_limit) * U256::from(max_fee_per_gas) + value,
            }),
            Transaction::Eip4844(TxEip4844 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                input,
                access_list,
                blob_versioned_hashes: _,
                max_fee_per_blob_gas,
            }) => Ok(Self::Eip4844 {
                chain_id,
                hash,
                sender,
                nonce,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                max_fee_per_blob_gas,
                gas_limit,
                to,
                value,
                input,
                access_list,
                sidecar: BlobTransactionSidecarVariant::Eip4844(BlobTransactionSidecar::default()),
                blob_versioned_hashes: Default::default(),
                size,
                cost: U256::from(gas_limit) * U256::from(max_fee_per_gas) + value,
            }),
            Transaction::Eip7702(TxEip7702 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                authorization_list,
                input,
            }) => Ok(Self::Eip7702 {
                chain_id,
                hash,
                sender,
                nonce,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                gas_limit,
                to,
                value,
                input,
                access_list,
                authorization_list,
                size,
                cost: U256::from(gas_limit) * U256::from(max_fee_per_gas) + value,
            }),
        }
    }
}

impl TryFrom<Recovered<EthereumTxEnvelope<TxEip4844Variant<BlobTransactionSidecarVariant>>>>
    for MockTransaction
{
    type Error = TryFromRecoveredTransactionError;

    fn try_from(
        tx: Recovered<EthereumTxEnvelope<TxEip4844Variant<BlobTransactionSidecarVariant>>>,
    ) -> Result<Self, Self::Error> {
        let sender = tx.signer();
        let transaction = tx.into_inner();
        let hash = *transaction.tx_hash();
        let size = transaction.size();

        match transaction {
            EthereumTxEnvelope::Legacy(signed_tx) => {
                let tx = signed_tx.strip_signature();
                Ok(Self::Legacy {
                    chain_id: tx.chain_id,
                    hash,
                    sender,
                    nonce: tx.nonce,
                    gas_price: tx.gas_price,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input,
                    size,
                    cost: U256::from(tx.gas_limit) * U256::from(tx.gas_price) + tx.value,
                })
            }
            EthereumTxEnvelope::Eip2930(signed_tx) => {
                let tx = signed_tx.strip_signature();
                Ok(Self::Eip2930 {
                    chain_id: tx.chain_id,
                    hash,
                    sender,
                    nonce: tx.nonce,
                    gas_price: tx.gas_price,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input,
                    access_list: tx.access_list,
                    size,
                    cost: U256::from(tx.gas_limit) * U256::from(tx.gas_price) + tx.value,
                })
            }
            EthereumTxEnvelope::Eip1559(signed_tx) => {
                let tx = signed_tx.strip_signature();
                Ok(Self::Eip1559 {
                    chain_id: tx.chain_id,
                    hash,
                    sender,
                    nonce: tx.nonce,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input,
                    access_list: tx.access_list,
                    size,
                    cost: U256::from(tx.gas_limit) * U256::from(tx.max_fee_per_gas) + tx.value,
                })
            }
            EthereumTxEnvelope::Eip4844(signed_tx) => match signed_tx.tx() {
                TxEip4844Variant::TxEip4844(tx) => Ok(Self::Eip4844 {
                    chain_id: tx.chain_id,
                    hash,
                    sender,
                    nonce: tx.nonce,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    max_fee_per_blob_gas: tx.max_fee_per_blob_gas,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input.clone(),
                    access_list: tx.access_list.clone(),
                    sidecar: BlobTransactionSidecarVariant::Eip4844(
                        BlobTransactionSidecar::default(),
                    ),
                    blob_versioned_hashes: tx.blob_versioned_hashes.clone(),
                    size,
                    cost: U256::from(tx.gas_limit) * U256::from(tx.max_fee_per_gas) + tx.value,
                }),
                tx => Err(TryFromRecoveredTransactionError::UnsupportedTransactionType(tx.ty())),
            },
            EthereumTxEnvelope::Eip7702(signed_tx) => {
                let tx = signed_tx.strip_signature();
                Ok(Self::Eip7702 {
                    chain_id: tx.chain_id,
                    hash,
                    sender,
                    nonce: tx.nonce,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    access_list: tx.access_list,
                    authorization_list: tx.authorization_list,
                    input: tx.input,
                    size,
                    cost: U256::from(tx.gas_limit) * U256::from(tx.max_fee_per_gas) + tx.value,
                })
            }
        }
    }
}

impl From<Recovered<PooledTransactionVariant>> for MockTransaction {
    fn from(tx: Recovered<PooledTransactionVariant>) -> Self {
        let (tx, signer) = tx.into_parts();
        Recovered::<TransactionSigned>::new_unchecked(tx.into(), signer).try_into().expect(
            "Failed to convert from PooledTransactionsElementEcRecovered to MockTransaction",
        )
    }
}

impl From<MockTransaction> for Recovered<TransactionSigned> {
    fn from(tx: MockTransaction) -> Self {
        let hash = *tx.hash();
        let sender = tx.sender();
        let tx = Transaction::from(tx);
        let tx: TransactionSigned =
            Signed::new_unchecked(tx, Signature::test_signature(), hash).into();
        Self::new_unchecked(tx, sender)
    }
}

impl From<MockTransaction> for Transaction {
    fn from(mock: MockTransaction) -> Self {
        match mock {
            MockTransaction::Legacy {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                input,
                ..
            } => Self::Legacy(TxLegacy { chain_id, nonce, gas_price, gas_limit, to, value, input }),
            MockTransaction::Eip2930 {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                access_list,
                input,
                ..
            } => Self::Eip2930(TxEip2930 {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                access_list,
                input,
            }),
            MockTransaction::Eip1559 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                input,
                ..
            } => Self::Eip1559(TxEip1559 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                input,
            }),
            MockTransaction::Eip4844 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                sidecar,
                max_fee_per_blob_gas,
                input,
                ..
            } => Self::Eip4844(TxEip4844 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                blob_versioned_hashes: sidecar.versioned_hashes().collect(),
                max_fee_per_blob_gas,
                input,
            }),
            MockTransaction::Eip7702 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                input,
                authorization_list,
                ..
            } => Self::Eip7702(TxEip7702 {
                chain_id,
                nonce,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                to,
                value,
                access_list,
                authorization_list,
                input,
            }),
        }
    }
}

#[cfg(any(test, feature = "arbitrary"))]
impl proptest::arbitrary::Arbitrary for MockTransaction {
    type Parameters = ();
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;
        use proptest_arbitrary_interop::arb;

        arb::<(TransactionSigned, Address)>()
            .prop_map(|(signed_transaction, signer)| {
                Recovered::new_unchecked(signed_transaction, signer)
                    .try_into()
                    .expect("Failed to create an Arbitrary MockTransaction from a Recovered tx")
            })
            .boxed()
    }

    type Strategy = proptest::strategy::BoxedStrategy<Self>;
}

/// A factory for creating and managing various types of mock transactions.
#[derive(Debug, Default)]
pub struct MockTransactionFactory {
    pub(crate) ids: SenderIdentifiers,
}

// === impl MockTransactionFactory ===

impl MockTransactionFactory {
    /// Generates a transaction ID for the given [`MockTransaction`].
    pub fn tx_id(&mut self, tx: &MockTransaction) -> TransactionId {
        let sender = self.ids.sender_id_or_create(tx.sender());
        TransactionId::new(sender, *tx.get_nonce())
    }

    /// Validates a [`MockTransaction`] and returns a [`MockValidTx`].
    pub fn validated(&mut self, transaction: MockTransaction) -> MockValidTx {
        self.validated_with_origin(TransactionOrigin::External, transaction)
    }

    /// Validates a [`MockTransaction`] and returns a shared [`Arc<MockValidTx>`].
    pub fn validated_arc(&mut self, transaction: MockTransaction) -> Arc<MockValidTx> {
        Arc::new(self.validated(transaction))
    }

    /// Converts the transaction into a validated transaction with a specified origin.
    pub fn validated_with_origin(
        &mut self,
        origin: TransactionOrigin,
        transaction: MockTransaction,
    ) -> MockValidTx {
        MockValidTx {
            propagate: false,
            transaction_id: self.tx_id(&transaction),
            transaction,
            timestamp: Instant::now(),
            origin,
            authority_ids: None,
        }
    }

    /// Creates a validated legacy [`MockTransaction`].
    pub fn create_legacy(&mut self) -> MockValidTx {
        self.validated(MockTransaction::legacy())
    }

    /// Creates a validated EIP-1559 [`MockTransaction`].
    pub fn create_eip1559(&mut self) -> MockValidTx {
        self.validated(MockTransaction::eip1559())
    }

    /// Creates a validated EIP-4844 [`MockTransaction`].
    pub fn create_eip4844(&mut self) -> MockValidTx {
        self.validated(MockTransaction::eip4844())
    }
}

/// `MockOrdering` is just a `CoinbaseTipOrdering` with `MockTransaction`
pub type MockOrdering = CoinbaseTipOrdering<MockTransaction>;

/// A ratio of each of the configured transaction types. The percentages sum up to 100, this is
/// enforced in [`MockTransactionRatio::new`] by an assert.
#[derive(Debug, Clone)]
pub struct MockTransactionRatio {
    /// Percent of transactions that are legacy transactions
    pub legacy_pct: u32,
    /// Percent of transactions that are access list transactions
    pub access_list_pct: u32,
    /// Percent of transactions that are EIP-1559 transactions
    pub dynamic_fee_pct: u32,
    /// Percent of transactions that are EIP-4844 transactions
    pub blob_pct: u32,
}

impl MockTransactionRatio {
    /// Creates a new [`MockTransactionRatio`] with the given percentages.
    ///
    /// Each argument is treated as a full percent, for example `30u32` is `30%`.
    ///
    /// The percentages must sum up to 100 exactly, or this method will panic.
    pub fn new(legacy_pct: u32, access_list_pct: u32, dynamic_fee_pct: u32, blob_pct: u32) -> Self {
        let total = legacy_pct + access_list_pct + dynamic_fee_pct + blob_pct;
        assert_eq!(
            total,
            100,
            "percentages must sum up to 100, instead got legacy: {legacy_pct}, access_list: {access_list_pct}, dynamic_fee: {dynamic_fee_pct}, blob: {blob_pct}, total: {total}",
        );

        Self { legacy_pct, access_list_pct, dynamic_fee_pct, blob_pct }
    }

    /// Create a [`WeightedIndex`] from this transaction ratio.
    ///
    /// This index will sample in the following order:
    /// * Legacy transaction => 0
    /// * EIP-2930 transaction => 1
    /// * EIP-1559 transaction => 2
    /// * EIP-4844 transaction => 3
    pub fn weighted_index(&self) -> WeightedIndex<u32> {
        WeightedIndex::new([
            self.legacy_pct,
            self.access_list_pct,
            self.dynamic_fee_pct,
            self.blob_pct,
        ])
        .unwrap()
    }
}

/// The range of each type of fee, for the different transaction types
#[derive(Debug, Clone)]
pub struct MockFeeRange {
    /// The range of `gas_price` or legacy and access list transactions
    pub gas_price: Uniform<u128>,
    /// The range of priority fees for EIP-1559 and EIP-4844 transactions
    pub priority_fee: Uniform<u128>,
    /// The range of max fees for EIP-1559 and EIP-4844 transactions
    pub max_fee: Uniform<u128>,
    /// The range of max fees per blob gas for EIP-4844 transactions
    pub max_fee_blob: Uniform<u128>,
}

impl MockFeeRange {
    /// Creates a new [`MockFeeRange`] with the given ranges.
    ///
    /// Expects the bottom of the `priority_fee_range` to be greater than the top of the
    /// `max_fee_range`.
    pub fn new(
        gas_price: Range<u128>,
        priority_fee: Range<u128>,
        max_fee: Range<u128>,
        max_fee_blob: Range<u128>,
    ) -> Self {
        assert!(
            max_fee.start <= priority_fee.end,
            "max_fee_range should be strictly below the priority fee range"
        );
        Self {
            gas_price: gas_price.try_into().unwrap(),
            priority_fee: priority_fee.try_into().unwrap(),
            max_fee: max_fee.try_into().unwrap(),
            max_fee_blob: max_fee_blob.try_into().unwrap(),
        }
    }

    /// Returns a sample of `gas_price` for legacy and access list transactions with the given
    /// [Rng](rand::Rng).
    pub fn sample_gas_price(&self, rng: &mut impl rand::Rng) -> u128 {
        self.gas_price.sample(rng)
    }

    /// Returns a sample of `max_priority_fee_per_gas` for EIP-1559 and EIP-4844 transactions with
    /// the given [Rng](rand::Rng).
    pub fn sample_priority_fee(&self, rng: &mut impl rand::Rng) -> u128 {
        self.priority_fee.sample(rng)
    }

    /// Returns a sample of `max_fee_per_gas` for EIP-1559 and EIP-4844 transactions with the given
    /// [Rng](rand::Rng).
    pub fn sample_max_fee(&self, rng: &mut impl rand::Rng) -> u128 {
        self.max_fee.sample(rng)
    }

    /// Returns a sample of `max_fee_per_blob_gas` for EIP-4844 transactions with the given
    /// [Rng](rand::Rng).
    pub fn sample_max_fee_blob(&self, rng: &mut impl rand::Rng) -> u128 {
        self.max_fee_blob.sample(rng)
    }
}

/// A configured distribution that can generate transactions
#[derive(Debug, Clone)]
pub struct MockTransactionDistribution {
    /// ratio of each transaction type to generate
    transaction_ratio: MockTransactionRatio,
    /// generates the gas limit
    gas_limit_range: Uniform<u64>,
    /// generates the transaction's fake size
    size_range: Uniform<usize>,
    /// generates fees for the given transaction types
    fee_ranges: MockFeeRange,
}

impl MockTransactionDistribution {
    /// Creates a new generator distribution.
    pub fn new(
        transaction_ratio: MockTransactionRatio,
        fee_ranges: MockFeeRange,
        gas_limit_range: Range<u64>,
        size_range: Range<usize>,
    ) -> Self {
        Self {
            transaction_ratio,
            gas_limit_range: gas_limit_range.try_into().unwrap(),
            fee_ranges,
            size_range: size_range.try_into().unwrap(),
        }
    }

    /// Generates a new transaction
    pub fn tx(&self, nonce: u64, rng: &mut impl rand::Rng) -> MockTransaction {
        let transaction_sample = self.transaction_ratio.weighted_index().sample(rng);
        let tx = match transaction_sample {
            0 => MockTransaction::legacy().with_gas_price(self.fee_ranges.sample_gas_price(rng)),
            1 => MockTransaction::eip2930().with_gas_price(self.fee_ranges.sample_gas_price(rng)),
            2 => MockTransaction::eip1559()
                .with_priority_fee(self.fee_ranges.sample_priority_fee(rng))
                .with_max_fee(self.fee_ranges.sample_max_fee(rng)),
            3 => MockTransaction::eip4844()
                .with_priority_fee(self.fee_ranges.sample_priority_fee(rng))
                .with_max_fee(self.fee_ranges.sample_max_fee(rng))
                .with_blob_fee(self.fee_ranges.sample_max_fee_blob(rng)),
            _ => unreachable!("unknown transaction type returned by the weighted index"),
        };

        let size = self.size_range.sample(rng);

        tx.with_nonce(nonce).with_gas_limit(self.gas_limit_range.sample(rng)).with_size(size)
    }

    /// Generates a new transaction set for the given sender.
    ///
    /// The nonce range defines which nonces to set, and how many transactions to generate.
    pub fn tx_set(
        &self,
        sender: Address,
        nonce_range: Range<u64>,
        rng: &mut impl rand::Rng,
    ) -> MockTransactionSet {
        let txs =
            nonce_range.map(|nonce| self.tx(nonce, rng).with_sender(sender)).collect::<Vec<_>>();
        MockTransactionSet::new(txs)
    }

    /// Generates a transaction set that ensures that blob txs are not mixed with other transaction
    /// types.
    ///
    /// This is done by taking the existing distribution, and using the first transaction to
    /// determine whether or not the sender should generate entirely blob transactions.
    pub fn tx_set_non_conflicting_types(
        &self,
        sender: Address,
        nonce_range: Range<u64>,
        rng: &mut impl rand::Rng,
    ) -> NonConflictingSetOutcome {
        // This will create a modified distribution that will only generate blob transactions
        // for the given sender, if the blob transaction is the first transaction in the set.
        //
        // Otherwise, it will modify the transaction distribution to only generate legacy, eip2930,
        // and eip1559 transactions.
        //
        // The new distribution should still have the same relative amount of transaction types.
        let mut modified_distribution = self.clone();
        let first_tx = self.tx(nonce_range.start, rng);

        // now we can check and modify the distribution, preserving potentially uneven ratios
        // between transaction types
        if first_tx.is_eip4844() {
            modified_distribution.transaction_ratio = MockTransactionRatio {
                legacy_pct: 0,
                access_list_pct: 0,
                dynamic_fee_pct: 0,
                blob_pct: 100,
            };

            // finally generate the transaction set
            NonConflictingSetOutcome::BlobsOnly(modified_distribution.tx_set(
                sender,
                nonce_range,
                rng,
            ))
        } else {
            let MockTransactionRatio { legacy_pct, access_list_pct, dynamic_fee_pct, .. } =
                modified_distribution.transaction_ratio;

            // Calculate the total weight of non-blob transactions
            let total_non_blob_weight: u32 = legacy_pct + access_list_pct + dynamic_fee_pct;

            // Calculate new weights, preserving the ratio between non-blob transaction types
            let new_weights: Vec<u32> = [legacy_pct, access_list_pct, dynamic_fee_pct]
                .into_iter()
                .map(|weight| weight * 100 / total_non_blob_weight)
                .collect();

            let new_ratio = MockTransactionRatio {
                legacy_pct: new_weights[0],
                access_list_pct: new_weights[1],
                dynamic_fee_pct: new_weights[2],
                blob_pct: 0,
            };

            // Set the new transaction ratio excluding blob transactions and preserving the relative
            // ratios
            modified_distribution.transaction_ratio = new_ratio;

            // finally generate the transaction set
            NonConflictingSetOutcome::Mixed(modified_distribution.tx_set(sender, nonce_range, rng))
        }
    }
}

/// Indicates whether or not the non-conflicting transaction set generated includes only blobs, or
/// a mix of transaction types.
#[derive(Debug, Clone)]
pub enum NonConflictingSetOutcome {
    /// The transaction set includes only blob transactions
    BlobsOnly(MockTransactionSet),
    /// The transaction set includes a mix of transaction types
    Mixed(MockTransactionSet),
}

impl NonConflictingSetOutcome {
    /// Returns the inner [`MockTransactionSet`]
    pub fn into_inner(self) -> MockTransactionSet {
        match self {
            Self::BlobsOnly(set) | Self::Mixed(set) => set,
        }
    }

    /// Introduces artificial nonce gaps into the transaction set, at random, with a range of gap
    /// sizes.
    ///
    /// If this is a [`NonConflictingSetOutcome::BlobsOnly`], then nonce gaps will not be
    /// introduced. Otherwise, the nonce gaps will be introduced to the mixed transaction set.
    ///
    /// See [`MockTransactionSet::with_nonce_gaps`] for more information on the generation process.
    pub fn with_nonce_gaps(
        &mut self,
        gap_pct: u32,
        gap_range: Range<u64>,
        rng: &mut impl rand::Rng,
    ) {
        match self {
            Self::BlobsOnly(_) => {}
            Self::Mixed(set) => set.with_nonce_gaps(gap_pct, gap_range, rng),
        }
    }
}

/// A set of [`MockTransaction`]s that can be modified at once
#[derive(Debug, Clone)]
pub struct MockTransactionSet {
    pub(crate) transactions: Vec<MockTransaction>,
}

impl MockTransactionSet {
    /// Create a new [`MockTransactionSet`] from a list of transactions
    const fn new(transactions: Vec<MockTransaction>) -> Self {
        Self { transactions }
    }

    /// Creates a series of dependent transactions for a given sender and nonce.
    ///
    /// This method generates a sequence of transactions starting from the provided nonce
    /// for the given sender.
    ///
    /// The number of transactions created is determined by `tx_count`.
    pub fn dependent(sender: Address, from_nonce: u64, tx_count: usize, tx_type: TxType) -> Self {
        let mut txs = Vec::with_capacity(tx_count);
        let mut curr_tx =
            MockTransaction::new_from_type(tx_type).with_nonce(from_nonce).with_sender(sender);
        for _ in 0..tx_count {
            txs.push(curr_tx.clone());
            curr_tx = curr_tx.next();
        }

        Self::new(txs)
    }

    /// Creates a chain of transactions for a given sender with a specified count.
    ///
    /// This method generates a sequence of transactions starting from the specified sender
    /// and creates a chain of transactions based on the `tx_count`.
    pub fn sequential_transactions_by_sender(
        sender: Address,
        tx_count: usize,
        tx_type: TxType,
    ) -> Self {
        Self::dependent(sender, 0, tx_count, tx_type)
    }

    /// Introduces artificial nonce gaps into the transaction set, at random, with a range of gap
    /// sizes.
    ///
    /// This assumes that the `gap_pct` is between 0 and 100, and the `gap_range` has a lower bound
    /// of at least one. This is enforced with assertions.
    ///
    /// The `gap_pct` is the percent chance that the next transaction in the set will introduce a
    /// nonce gap.
    ///
    /// Let an example transaction set be `[(tx1, 1), (tx2, 2)]`, where the first element of the
    /// tuple is a transaction, and the second element is the nonce. If the `gap_pct` is 50, and
    /// the `gap_range` is `1..=1`, then the resulting transaction set could would be either
    /// `[(tx1, 1), (tx2, 2)]` or `[(tx1, 1), (tx2, 3)]`, with a 50% chance of either.
    pub fn with_nonce_gaps(
        &mut self,
        gap_pct: u32,
        gap_range: Range<u64>,
        rng: &mut impl rand::Rng,
    ) {
        assert!(gap_pct <= 100, "gap_pct must be between 0 and 100");
        assert!(gap_range.start >= 1, "gap_range must have a lower bound of at least one");

        let mut prev_nonce = 0;
        for tx in &mut self.transactions {
            if rng.random_bool(gap_pct as f64 / 100.0) {
                prev_nonce += gap_range.start;
            } else {
                prev_nonce += 1;
            }
            tx.set_nonce(prev_nonce);
        }
    }

    /// Add transactions to the [`MockTransactionSet`]
    pub fn extend<T: IntoIterator<Item = MockTransaction>>(&mut self, txs: T) {
        self.transactions.extend(txs);
    }

    /// Extract the inner [Vec] of [`MockTransaction`]s
    pub fn into_vec(self) -> Vec<MockTransaction> {
        self.transactions
    }

    /// Returns an iterator over the contained transactions in the set
    pub fn iter(&self) -> impl Iterator<Item = &MockTransaction> {
        self.transactions.iter()
    }

    /// Returns a mutable iterator over the contained transactions in the set.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut MockTransaction> {
        self.transactions.iter_mut()
    }
}

impl IntoIterator for MockTransactionSet {
    type Item = MockTransaction;
    type IntoIter = IntoIter<MockTransaction>;

    fn into_iter(self) -> Self::IntoIter {
        self.transactions.into_iter()
    }
}

#[test]
fn test_mock_priority() {
    use crate::TransactionOrdering;

    let o = MockOrdering::default();
    let lo = MockTransaction::eip1559().with_gas_limit(100_000);
    let hi = lo.next().inc_price();
    assert!(o.priority(&hi, 0) > o.priority(&lo, 0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Transaction;
    use alloy_primitives::U256;

    #[test]
    fn test_mock_transaction_factory() {
        let mut factory = MockTransactionFactory::default();

        // Test legacy transaction creation
        let legacy = factory.create_legacy();
        assert_eq!(legacy.transaction.tx_type(), TxType::Legacy);

        // Test EIP1559 transaction creation
        let eip1559 = factory.create_eip1559();
        assert_eq!(eip1559.transaction.tx_type(), TxType::Eip1559);

        // Test EIP4844 transaction creation
        let eip4844 = factory.create_eip4844();
        assert_eq!(eip4844.transaction.tx_type(), TxType::Eip4844);
    }

    #[test]
    fn test_mock_transaction_set() {
        let sender = Address::random();
        let nonce_start = 0u64;
        let count = 3;

        // Test legacy transaction set
        let legacy_set = MockTransactionSet::dependent(sender, nonce_start, count, TxType::Legacy);
        assert_eq!(legacy_set.transactions.len(), count);
        for (idx, tx) in legacy_set.transactions.iter().enumerate() {
            assert_eq!(tx.tx_type(), TxType::Legacy);
            assert_eq!(tx.nonce(), nonce_start + idx as u64);
            assert_eq!(tx.sender(), sender);
        }

        // Test EIP1559 transaction set
        let eip1559_set =
            MockTransactionSet::dependent(sender, nonce_start, count, TxType::Eip1559);
        assert_eq!(eip1559_set.transactions.len(), count);
        for (idx, tx) in eip1559_set.transactions.iter().enumerate() {
            assert_eq!(tx.tx_type(), TxType::Eip1559);
            assert_eq!(tx.nonce(), nonce_start + idx as u64);
            assert_eq!(tx.sender(), sender);
        }
    }

    #[test]
    fn test_mock_transaction_modifications() {
        let tx = MockTransaction::eip1559();

        // Test price increment
        let original_price = tx.get_gas_price();
        let tx_inc = tx.inc_price();
        assert!(tx_inc.get_gas_price() > original_price);

        // Test gas limit increment
        let original_limit = tx.gas_limit();
        let tx_inc = tx.inc_limit();
        assert!(tx_inc.gas_limit() > original_limit);

        // Test nonce increment
        let original_nonce = tx.nonce();
        let tx_inc = tx.inc_nonce();
        assert_eq!(tx_inc.nonce(), original_nonce + 1);
    }

    #[test]
    fn test_mock_transaction_cost() {
        let tx = MockTransaction::eip1559()
            .with_gas_limit(7_000)
            .with_max_fee(100)
            .with_value(U256::ZERO);

        // Cost is calculated as (gas_limit * max_fee_per_gas) + value
        let expected_cost = U256::from(7_000u64) * U256::from(100u128) + U256::ZERO;
        assert_eq!(*tx.cost(), expected_cost);
    }
}
