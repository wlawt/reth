//! Implementation specific Errors for the `eth_` namespace.

pub mod api;
use crate::error::api::FromEvmHalt;
use alloy_eips::BlockId;
use alloy_evm::{call::CallError, overrides::StateOverrideError};
use alloy_primitives::{Address, Bytes, B256, U256};
use alloy_rpc_types_eth::{error::EthRpcErrorCode, request::TransactionInputError, BlockError};
use alloy_sol_types::{ContractError, RevertReason};
pub use api::{AsEthApiError, FromEthApiError, FromEvmError, IntoEthApiError};
use core::time::Duration;
use reth_errors::{BlockExecutionError, BlockValidationError, RethError};
use reth_primitives_traits::transaction::{error::InvalidTransactionError, signed::RecoveryError};
use reth_rpc_convert::{CallFeesError, EthTxEnvError, TransactionConversionError};
use reth_rpc_server_types::result::{
    block_id_to_str, internal_rpc_err, invalid_params_rpc_err, rpc_err, rpc_error_with_code,
};
use reth_transaction_pool::error::{
    Eip4844PoolTransactionError, Eip7702PoolTransactionError, InvalidPoolTransactionError,
    PoolError, PoolErrorKind, PoolTransactionError,
};
use revm::context_interface::result::{
    EVMError, ExecutionResult, HaltReason, InvalidHeader, InvalidTransaction, OutOfGasError,
};
use revm_inspectors::tracing::MuxError;
use std::convert::Infallible;
use tracing::error;

/// A trait to convert an error to an RPC error.
pub trait ToRpcError: core::error::Error + Send + Sync + 'static {
    /// Converts the error to a JSON-RPC error object.
    fn to_rpc_error(&self) -> jsonrpsee_types::ErrorObject<'static>;
}

impl ToRpcError for jsonrpsee_types::ErrorObject<'static> {
    fn to_rpc_error(&self) -> jsonrpsee_types::ErrorObject<'static> {
        self.clone()
    }
}

/// Result alias
pub type EthResult<T> = Result<T, EthApiError>;

/// Errors that can occur when interacting with the `eth_` namespace
#[derive(Debug, thiserror::Error)]
pub enum EthApiError {
    /// When a raw transaction is empty
    #[error("empty transaction data")]
    EmptyRawTransactionData,
    /// When decoding a signed transaction fails
    #[error("failed to decode signed transaction")]
    FailedToDecodeSignedTransaction,
    /// When the transaction signature is invalid
    #[error("invalid transaction signature")]
    InvalidTransactionSignature,
    /// Errors related to the transaction pool
    #[error(transparent)]
    PoolError(RpcPoolError),
    /// Header not found for block hash/number/tag
    #[error("header not found")]
    HeaderNotFound(BlockId),
    /// Header range not found for start block hash/number/tag to end block hash/number/tag
    #[error("header range not found, start block {0:?}, end block {1:?}")]
    HeaderRangeNotFound(BlockId, BlockId),
    /// Thrown when historical data is not available because it has been pruned
    ///
    /// This error is intended for use as a standard response when historical data is
    /// requested that has been pruned according to the node's data retention policy.
    ///
    /// See also <https://eips.ethereum.org/EIPS/eip-4444>
    #[error("pruned history unavailable")]
    PrunedHistoryUnavailable,
    /// Receipts not found for block hash/number/tag
    #[error("receipts not found")]
    ReceiptsNotFound(BlockId),
    /// Thrown when an unknown block or transaction index is encountered
    #[error("unknown block or tx index")]
    UnknownBlockOrTxIndex,
    /// When an invalid block range is provided
    #[error("invalid block range")]
    InvalidBlockRange,
    /// Thrown when the target block for proof computation exceeds the maximum configured window.
    #[error("distance to target block exceeds maximum proof window")]
    ExceedsMaxProofWindow,
    /// An internal error where prevrandao is not set in the evm's environment
    #[error("prevrandao not in the EVM's environment after merge")]
    PrevrandaoNotSet,
    /// `excess_blob_gas` is not set for Cancun and above
    #[error("excess blob gas missing in the EVM's environment after Cancun")]
    ExcessBlobGasNotSet,
    /// Thrown when a call or transaction request (`eth_call`, `eth_estimateGas`,
    /// `eth_sendTransaction`) contains conflicting fields (legacy, EIP-1559)
    #[error("both gasPrice and (maxFeePerGas or maxPriorityFeePerGas) specified")]
    ConflictingFeeFieldsInRequest,
    /// Errors related to invalid transactions
    #[error(transparent)]
    InvalidTransaction(#[from] RpcInvalidTransactionError),
    /// Thrown when constructing an RPC block from primitive block data fails
    #[error(transparent)]
    InvalidBlockData(#[from] BlockError),
    /// Thrown when an `AccountOverride` contains conflicting `state` and `stateDiff` fields
    #[error("account {0:?} has both 'state' and 'stateDiff'")]
    BothStateAndStateDiffInOverride(Address),
    /// Other internal error
    #[error(transparent)]
    Internal(RethError),
    /// Error related to signing
    #[error(transparent)]
    Signing(#[from] SignError),
    /// Thrown when a requested transaction is not found
    #[error("transaction not found")]
    TransactionNotFound,
    /// Some feature is unsupported
    #[error("unsupported")]
    Unsupported(&'static str),
    /// General purpose error for invalid params
    #[error("{0}")]
    InvalidParams(String),
    /// When the tracer config does not match the tracer
    #[error("invalid tracer config")]
    InvalidTracerConfig,
    /// When the percentile array is invalid
    #[error("invalid reward percentiles")]
    InvalidRewardPercentiles,
    /// Error thrown when a spawned blocking task failed to deliver an anticipated response.
    ///
    /// This only happens if the blocking task panics and is aborted before it can return a
    /// response back to the request handler.
    #[error("internal blocking task error")]
    InternalBlockingTaskError,
    /// Error thrown when a spawned blocking task failed to deliver an anticipated response
    #[error("internal eth error")]
    InternalEthError,
    /// Error thrown when a (tracing) call exceeds the configured timeout
    #[error("execution aborted (timeout = {0:?})")]
    ExecutionTimedOut(Duration),
    /// Internal Error thrown by the javascript tracer
    #[error("{0}")]
    InternalJsTracerError(String),
    #[error(transparent)]
    /// Call Input error when both `data` and `input` fields are set and not equal.
    TransactionInputError(#[from] TransactionInputError),
    /// Evm generic purpose error.
    #[error("Revm error: {0}")]
    EvmCustom(String),
    /// Bytecode override is invalid.
    ///
    /// This can happen if bytecode provided in an
    /// [`AccountOverride`](alloy_rpc_types_eth::state::AccountOverride) is malformed, e.g. invalid
    /// 7702 bytecode.
    #[error("Invalid bytecode: {0}")]
    InvalidBytecode(String),
    /// Error encountered when converting a transaction type
    #[error("Transaction conversion error")]
    TransactionConversionError,
    /// Error thrown when tracing with a muxTracer fails
    #[error(transparent)]
    MuxTracerError(#[from] MuxError),
    /// Error thrown when waiting for transaction confirmation times out
    #[error(
        "Transaction {hash} was added to the mempool but wasn't confirmed within {duration:?}."
    )]
    TransactionConfirmationTimeout {
        /// Hash of the transaction that timed out
        hash: B256,
        /// Duration that was waited before timing out
        duration: Duration,
    },
    /// Any other error
    #[error("{0}")]
    Other(Box<dyn ToRpcError>),
}

impl EthApiError {
    /// crates a new [`EthApiError::Other`] variant.
    pub fn other<E: ToRpcError>(err: E) -> Self {
        Self::Other(Box::new(err))
    }

    /// Returns `true` if error is [`RpcInvalidTransactionError::GasTooHigh`]
    pub const fn is_gas_too_high(&self) -> bool {
        matches!(self, Self::InvalidTransaction(RpcInvalidTransactionError::GasTooHigh))
    }

    /// Returns `true` if error is [`RpcInvalidTransactionError::GasTooLow`]
    pub const fn is_gas_too_low(&self) -> bool {
        matches!(self, Self::InvalidTransaction(RpcInvalidTransactionError::GasTooLow))
    }

    /// Returns the [`RpcInvalidTransactionError`] if this is a [`EthApiError::InvalidTransaction`]
    pub const fn as_invalid_transaction(&self) -> Option<&RpcInvalidTransactionError> {
        match self {
            Self::InvalidTransaction(e) => Some(e),
            _ => None,
        }
    }

    /// Converts the given [`StateOverrideError`] into a new [`EthApiError`] instance.
    pub fn from_state_overrides_err<E>(err: StateOverrideError<E>) -> Self
    where
        E: Into<Self>,
    {
        err.into()
    }

    /// Converts the given [`CallError`] into a new [`EthApiError`] instance.
    pub fn from_call_err<E>(err: CallError<E>) -> Self
    where
        E: Into<Self>,
    {
        err.into()
    }

    /// Converts this error into the rpc error object.
    pub fn into_rpc_err(self) -> jsonrpsee_types::error::ErrorObject<'static> {
        self.into()
    }
}

impl From<EthApiError> for jsonrpsee_types::error::ErrorObject<'static> {
    fn from(error: EthApiError) -> Self {
        match error {
            EthApiError::FailedToDecodeSignedTransaction |
            EthApiError::InvalidTransactionSignature |
            EthApiError::EmptyRawTransactionData |
            EthApiError::InvalidBlockRange |
            EthApiError::ExceedsMaxProofWindow |
            EthApiError::ConflictingFeeFieldsInRequest |
            EthApiError::Signing(_) |
            EthApiError::BothStateAndStateDiffInOverride(_) |
            EthApiError::InvalidTracerConfig |
            EthApiError::TransactionConversionError |
            EthApiError::InvalidRewardPercentiles |
            EthApiError::InvalidBytecode(_) => invalid_params_rpc_err(error.to_string()),
            EthApiError::InvalidTransaction(err) => err.into(),
            EthApiError::PoolError(err) => err.into(),
            EthApiError::PrevrandaoNotSet |
            EthApiError::ExcessBlobGasNotSet |
            EthApiError::InvalidBlockData(_) |
            EthApiError::Internal(_) |
            EthApiError::EvmCustom(_) => internal_rpc_err(error.to_string()),
            EthApiError::UnknownBlockOrTxIndex | EthApiError::TransactionNotFound => {
                rpc_error_with_code(EthRpcErrorCode::ResourceNotFound.code(), error.to_string())
            }
            // TODO(onbjerg): We rewrite the error message here because op-node does string matching
            // on the error message.
            //
            // Until https://github.com/ethereum-optimism/optimism/pull/11759 is released, this must be kept around.
            EthApiError::HeaderNotFound(id) => rpc_error_with_code(
                EthRpcErrorCode::ResourceNotFound.code(),
                format!("block not found: {}", block_id_to_str(id)),
            ),
            EthApiError::ReceiptsNotFound(id) => rpc_error_with_code(
                EthRpcErrorCode::ResourceNotFound.code(),
                format!("{error}: {}", block_id_to_str(id)),
            ),
            EthApiError::HeaderRangeNotFound(start_id, end_id) => rpc_error_with_code(
                EthRpcErrorCode::ResourceNotFound.code(),
                format!(
                    "{error}: start block: {}, end block: {}",
                    block_id_to_str(start_id),
                    block_id_to_str(end_id),
                ),
            ),
            err @ EthApiError::TransactionConfirmationTimeout { .. } => {
                rpc_error_with_code(EthRpcErrorCode::TransactionRejected.code(), err.to_string())
            }
            EthApiError::Unsupported(msg) => internal_rpc_err(msg),
            EthApiError::InternalJsTracerError(msg) => internal_rpc_err(msg),
            EthApiError::InvalidParams(msg) => invalid_params_rpc_err(msg),
            err @ EthApiError::ExecutionTimedOut(_) => rpc_error_with_code(
                jsonrpsee_types::error::CALL_EXECUTION_FAILED_CODE,
                err.to_string(),
            ),
            err @ (EthApiError::InternalBlockingTaskError | EthApiError::InternalEthError) => {
                internal_rpc_err(err.to_string())
            }
            err @ EthApiError::TransactionInputError(_) => invalid_params_rpc_err(err.to_string()),
            EthApiError::PrunedHistoryUnavailable => rpc_error_with_code(4444, error.to_string()),
            EthApiError::Other(err) => err.to_rpc_error(),
            EthApiError::MuxTracerError(msg) => internal_rpc_err(msg.to_string()),
        }
    }
}

impl From<TransactionConversionError> for EthApiError {
    fn from(_: TransactionConversionError) -> Self {
        Self::TransactionConversionError
    }
}

impl<E> From<CallError<E>> for EthApiError
where
    E: Into<Self>,
{
    fn from(value: CallError<E>) -> Self {
        match value {
            CallError::Database(err) => err.into(),
            CallError::InsufficientFunds(insufficient_funds_error) => {
                Self::InvalidTransaction(RpcInvalidTransactionError::InsufficientFunds {
                    cost: insufficient_funds_error.cost,
                    balance: insufficient_funds_error.balance,
                })
            }
        }
    }
}

impl<E> From<StateOverrideError<E>> for EthApiError
where
    E: Into<Self>,
{
    fn from(value: StateOverrideError<E>) -> Self {
        match value {
            StateOverrideError::InvalidBytecode(bytecode_decode_error) => {
                Self::InvalidBytecode(bytecode_decode_error.to_string())
            }
            StateOverrideError::BothStateAndStateDiff(address) => {
                Self::BothStateAndStateDiffInOverride(address)
            }
            StateOverrideError::Database(err) => err.into(),
        }
    }
}

impl From<EthTxEnvError> for EthApiError {
    fn from(value: EthTxEnvError) -> Self {
        match value {
            EthTxEnvError::CallFees(CallFeesError::BlobTransactionMissingBlobHashes) => {
                Self::InvalidTransaction(
                    RpcInvalidTransactionError::BlobTransactionMissingBlobHashes,
                )
            }
            EthTxEnvError::CallFees(CallFeesError::FeeCapTooLow) => {
                Self::InvalidTransaction(RpcInvalidTransactionError::FeeCapTooLow)
            }
            EthTxEnvError::CallFees(CallFeesError::ConflictingFeeFieldsInRequest) => {
                Self::ConflictingFeeFieldsInRequest
            }
            EthTxEnvError::CallFees(CallFeesError::TipAboveFeeCap) => {
                Self::InvalidTransaction(RpcInvalidTransactionError::TipAboveFeeCap)
            }
            EthTxEnvError::CallFees(CallFeesError::TipVeryHigh) => {
                Self::InvalidTransaction(RpcInvalidTransactionError::TipVeryHigh)
            }
            EthTxEnvError::Input(err) => Self::TransactionInputError(err),
        }
    }
}

#[cfg(feature = "js-tracer")]
impl From<revm_inspectors::tracing::js::JsInspectorError> for EthApiError {
    fn from(error: revm_inspectors::tracing::js::JsInspectorError) -> Self {
        match error {
            err @ revm_inspectors::tracing::js::JsInspectorError::JsError(_) => {
                Self::InternalJsTracerError(err.to_string())
            }
            err => Self::InvalidParams(err.to_string()),
        }
    }
}

impl From<RethError> for EthApiError {
    fn from(error: RethError) -> Self {
        match error {
            RethError::Provider(err) => err.into(),
            err => Self::Internal(err),
        }
    }
}

impl From<BlockExecutionError> for EthApiError {
    fn from(error: BlockExecutionError) -> Self {
        match error {
            BlockExecutionError::Validation(validation_error) => match validation_error {
                BlockValidationError::InvalidTx { error, .. } => {
                    if let Some(invalid_tx) = error.as_invalid_tx_err() {
                        Self::InvalidTransaction(RpcInvalidTransactionError::from(
                            invalid_tx.clone(),
                        ))
                    } else {
                        Self::InvalidTransaction(RpcInvalidTransactionError::other(
                            rpc_error_with_code(
                                EthRpcErrorCode::TransactionRejected.code(),
                                error.to_string(),
                            ),
                        ))
                    }
                }
                _ => Self::Internal(RethError::Execution(BlockExecutionError::Validation(
                    validation_error,
                ))),
            },
            BlockExecutionError::Internal(internal_error) => {
                Self::Internal(RethError::Execution(BlockExecutionError::Internal(internal_error)))
            }
        }
    }
}

impl From<reth_errors::ProviderError> for EthApiError {
    fn from(error: reth_errors::ProviderError) -> Self {
        use reth_errors::ProviderError;
        match error {
            ProviderError::HeaderNotFound(hash) => Self::HeaderNotFound(hash.into()),
            ProviderError::BlockHashNotFound(hash) | ProviderError::UnknownBlockHash(hash) => {
                Self::HeaderNotFound(hash.into())
            }
            ProviderError::BestBlockNotFound => Self::HeaderNotFound(BlockId::latest()),
            ProviderError::BlockNumberForTransactionIndexNotFound => Self::UnknownBlockOrTxIndex,
            ProviderError::TotalDifficultyNotFound(num) => Self::HeaderNotFound(num.into()),
            ProviderError::FinalizedBlockNotFound => Self::HeaderNotFound(BlockId::finalized()),
            ProviderError::SafeBlockNotFound => Self::HeaderNotFound(BlockId::safe()),
            err => Self::Internal(err.into()),
        }
    }
}

impl From<InvalidHeader> for EthApiError {
    fn from(value: InvalidHeader) -> Self {
        match value {
            InvalidHeader::ExcessBlobGasNotSet => Self::ExcessBlobGasNotSet,
            InvalidHeader::PrevrandaoNotSet => Self::PrevrandaoNotSet,
        }
    }
}

impl<T> From<EVMError<T, InvalidTransaction>> for EthApiError
where
    T: Into<Self>,
{
    fn from(err: EVMError<T, InvalidTransaction>) -> Self {
        match err {
            EVMError::Transaction(invalid_tx) => match invalid_tx {
                InvalidTransaction::NonceTooLow { tx, state } => {
                    Self::InvalidTransaction(RpcInvalidTransactionError::NonceTooLow { tx, state })
                }
                _ => RpcInvalidTransactionError::from(invalid_tx).into(),
            },
            EVMError::Header(err) => err.into(),
            EVMError::Database(err) => err.into(),
            EVMError::Custom(err) => Self::EvmCustom(err),
        }
    }
}

impl From<RecoveryError> for EthApiError {
    fn from(_: RecoveryError) -> Self {
        Self::InvalidTransactionSignature
    }
}

impl From<Infallible> for EthApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

/// An error due to invalid transaction.
///
/// The only reason this exists is to maintain compatibility with other clients de-facto standard
/// error messages.
///
/// These error variants can be thrown when the transaction is checked prior to execution.
///
/// These variants also cover all errors that can be thrown by revm.
///
/// ## Nomenclature
///
/// This type is explicitly modeled after geth's error variants and uses
///   `fee cap` for `max_fee_per_gas`
///   `tip` for `max_priority_fee_per_gas`
#[derive(thiserror::Error, Debug)]
pub enum RpcInvalidTransactionError {
    /// returned if the nonce of a transaction is lower than the one present in the local chain.
    #[error("nonce too low: next nonce {state}, tx nonce {tx}")]
    NonceTooLow {
        /// The nonce of the transaction.
        tx: u64,
        /// The current state of the nonce in the local chain.
        state: u64,
    },
    /// returned if the nonce of a transaction is higher than the next one expected based on the
    /// local chain.
    #[error("nonce too high")]
    NonceTooHigh,
    /// Returned if the nonce of a transaction is too high
    /// Incrementing the nonce would lead to invalid state (overflow)
    #[error("nonce has max value")]
    NonceMaxValue,
    /// thrown if the transaction sender doesn't have enough funds for a transfer
    #[error("insufficient funds for transfer")]
    InsufficientFundsForTransfer,
    /// thrown if creation transaction provides the init code bigger than init code size limit.
    #[error("max initcode size exceeded")]
    MaxInitCodeSizeExceeded,
    /// Represents the inability to cover max fee + value (account balance too low).
    #[error("insufficient funds for gas * price + value: have {balance} want {cost}")]
    InsufficientFunds {
        /// Transaction cost.
        cost: U256,
        /// Current balance of transaction sender.
        balance: U256,
    },
    /// This is similar to [`Self::InsufficientFunds`] but with a different error message and
    /// exists for compatibility reasons.
    ///
    /// This error is used in `eth_estimateCall` when the highest available gas limit, capped with
    /// the allowance of the caller is too low: [`Self::GasTooLow`].
    #[error("gas required exceeds allowance ({gas_limit})")]
    GasRequiredExceedsAllowance {
        /// The gas limit the transaction was executed with.
        gas_limit: u64,
    },
    /// Thrown when calculating gas usage
    #[error("gas uint64 overflow")]
    GasUintOverflow,
    /// Thrown if the transaction is specified to use less gas than required to start the
    /// invocation.
    #[error("intrinsic gas too low")]
    GasTooLow,
    /// Thrown if the transaction gas exceeds the limit
    #[error("intrinsic gas too high")]
    GasTooHigh,
    /// Thrown if the transaction gas limit exceeds the maximum
    #[error("gas limit too high")]
    GasLimitTooHigh,
    /// Thrown if a transaction is not supported in the current network configuration.
    #[error("transaction type not supported")]
    TxTypeNotSupported,
    /// Thrown to ensure no one is able to specify a transaction with a tip higher than the total
    /// fee cap.
    #[error("max priority fee per gas higher than max fee per gas")]
    TipAboveFeeCap,
    /// A sanity error to avoid huge numbers specified in the tip field.
    #[error("max priority fee per gas higher than 2^256-1")]
    TipVeryHigh,
    /// A sanity error to avoid huge numbers specified in the fee cap field.
    #[error("max fee per gas higher than 2^256-1")]
    FeeCapVeryHigh,
    /// Thrown post London if the transaction's fee is less than the base fee of the block
    #[error("max fee per gas less than block base fee")]
    FeeCapTooLow,
    /// Thrown if the sender of a transaction is a contract.
    #[error("sender is not an EOA")]
    SenderNoEOA,
    /// Gas limit was exceeded during execution.
    /// Contains the gas limit.
    #[error("out of gas: gas required exceeds: {0}")]
    BasicOutOfGas(u64),
    /// Gas limit was exceeded during memory expansion.
    /// Contains the gas limit.
    #[error("out of gas: gas exhausted during memory expansion: {0}")]
    MemoryOutOfGas(u64),
    /// Gas limit was exceeded during precompile execution.
    /// Contains the gas limit.
    #[error("out of gas: gas exhausted during precompiled contract execution: {0}")]
    PrecompileOutOfGas(u64),
    /// An operand to an opcode was invalid or out of range.
    /// Contains the gas limit.
    #[error("out of gas: invalid operand to an opcode: {0}")]
    InvalidOperandOutOfGas(u64),
    /// Thrown if executing a transaction failed during estimate/call
    #[error(transparent)]
    Revert(RevertError),
    /// Unspecific EVM halt error.
    #[error("EVM error: {0:?}")]
    EvmHalt(HaltReason),
    /// Invalid chain id set for the transaction.
    #[error("invalid chain ID")]
    InvalidChainId,
    /// The transaction is before Spurious Dragon and has a chain ID
    #[error("transactions before Spurious Dragon should not have a chain ID")]
    OldLegacyChainId,
    /// The transitions is before Berlin and has access list
    #[error("transactions before Berlin should not have access list")]
    AccessListNotSupported,
    /// `max_fee_per_blob_gas` is not supported for blocks before the Cancun hardfork.
    #[error("max_fee_per_blob_gas is not supported for blocks before the Cancun hardfork")]
    MaxFeePerBlobGasNotSupported,
    /// `blob_hashes`/`blob_versioned_hashes` is not supported for blocks before the Cancun
    /// hardfork.
    #[error("blob_versioned_hashes is not supported for blocks before the Cancun hardfork")]
    BlobVersionedHashesNotSupported,
    /// Block `blob_base_fee` is greater than tx-specified `max_fee_per_blob_gas` after Cancun.
    #[error("max fee per blob gas less than block blob gas fee")]
    BlobFeeCapTooLow,
    /// Blob transaction has a versioned hash with an invalid blob
    #[error("blob hash version mismatch")]
    BlobHashVersionMismatch,
    /// Blob transaction has no versioned hashes
    #[error("blob transaction missing blob hashes")]
    BlobTransactionMissingBlobHashes,
    /// Blob transaction has too many blobs
    #[error("blob transaction exceeds max blobs per block; got {have}")]
    TooManyBlobs {
        /// The number of blobs in the transaction.
        have: usize,
    },
    /// Blob transaction is a create transaction
    #[error("blob transaction is a create transaction")]
    BlobTransactionIsCreate,
    /// EIP-7702 is not enabled.
    #[error("EIP-7702 authorization list not supported")]
    AuthorizationListNotSupported,
    /// EIP-7702 transaction has invalid fields set.
    #[error("EIP-7702 authorization list has invalid fields")]
    AuthorizationListInvalidFields,
    /// Transaction priority fee is below the minimum required priority fee.
    #[error("transaction priority fee below minimum required priority fee {minimum_priority_fee}")]
    PriorityFeeBelowMinimum {
        /// Minimum required priority fee.
        minimum_priority_fee: u128,
    },
    /// Any other error
    #[error("{0}")]
    Other(Box<dyn ToRpcError>),
}

impl RpcInvalidTransactionError {
    /// crates a new [`RpcInvalidTransactionError::Other`] variant.
    pub fn other<E: ToRpcError>(err: E) -> Self {
        Self::Other(Box::new(err))
    }

    /// Returns the rpc error code for this error.
    pub const fn error_code(&self) -> i32 {
        match self {
            Self::InvalidChainId |
            Self::GasTooLow |
            Self::GasTooHigh |
            Self::GasRequiredExceedsAllowance { .. } |
            Self::NonceTooLow { .. } |
            Self::NonceTooHigh { .. } |
            Self::FeeCapTooLow |
            Self::FeeCapVeryHigh => EthRpcErrorCode::InvalidInput.code(),
            Self::Revert(_) => EthRpcErrorCode::ExecutionError.code(),
            _ => EthRpcErrorCode::TransactionRejected.code(),
        }
    }

    /// Converts the halt error
    ///
    /// Takes the configured gas limit of the transaction which is attached to the error
    pub const fn halt(reason: HaltReason, gas_limit: u64) -> Self {
        match reason {
            HaltReason::OutOfGas(err) => Self::out_of_gas(err, gas_limit),
            HaltReason::NonceOverflow => Self::NonceMaxValue,
            err => Self::EvmHalt(err),
        }
    }

    /// Converts the out of gas error
    pub const fn out_of_gas(reason: OutOfGasError, gas_limit: u64) -> Self {
        match reason {
            OutOfGasError::Basic | OutOfGasError::ReentrancySentry => {
                Self::BasicOutOfGas(gas_limit)
            }
            OutOfGasError::Memory | OutOfGasError::MemoryLimit => Self::MemoryOutOfGas(gas_limit),
            OutOfGasError::Precompile => Self::PrecompileOutOfGas(gas_limit),
            OutOfGasError::InvalidOperand => Self::InvalidOperandOutOfGas(gas_limit),
        }
    }

    /// Converts this error into the rpc error object.
    pub fn into_rpc_err(self) -> jsonrpsee_types::error::ErrorObject<'static> {
        self.into()
    }
}

impl From<RpcInvalidTransactionError> for jsonrpsee_types::error::ErrorObject<'static> {
    fn from(err: RpcInvalidTransactionError) -> Self {
        match err {
            RpcInvalidTransactionError::Revert(revert) => {
                // include out data if some
                rpc_err(
                    revert.error_code(),
                    revert.to_string(),
                    revert.output.as_ref().map(|out| out.as_ref()),
                )
            }
            RpcInvalidTransactionError::Other(err) => err.to_rpc_error(),
            err => rpc_err(err.error_code(), err.to_string(), None),
        }
    }
}

impl From<InvalidTransaction> for RpcInvalidTransactionError {
    fn from(err: InvalidTransaction) -> Self {
        match err {
            InvalidTransaction::InvalidChainId | InvalidTransaction::MissingChainId => {
                Self::InvalidChainId
            }
            InvalidTransaction::PriorityFeeGreaterThanMaxFee => Self::TipAboveFeeCap,
            InvalidTransaction::GasPriceLessThanBasefee => Self::FeeCapTooLow,
            InvalidTransaction::CallerGasLimitMoreThanBlock |
            InvalidTransaction::TxGasLimitGreaterThanCap { .. } => {
                // tx.gas > block.gas_limit
                Self::GasTooHigh
            }
            InvalidTransaction::CallGasCostMoreThanGasLimit { .. } => {
                // tx.gas < cost
                Self::GasTooLow
            }
            InvalidTransaction::GasFloorMoreThanGasLimit { .. } => {
                // Post prague EIP-7623 tx floor calldata gas cost > tx.gas_limit
                // where floor gas is the minimum amount of gas that will be spent
                // In other words, the tx's gas limit is lower that the minimum gas requirements of
                // the tx's calldata
                Self::GasTooLow
            }
            InvalidTransaction::RejectCallerWithCode => Self::SenderNoEOA,
            InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                Self::InsufficientFunds { cost: *fee, balance: *balance }
            }
            InvalidTransaction::OverflowPaymentInTransaction => Self::GasUintOverflow,
            InvalidTransaction::NonceOverflowInTransaction => Self::NonceMaxValue,
            InvalidTransaction::CreateInitCodeSizeLimit => Self::MaxInitCodeSizeExceeded,
            InvalidTransaction::NonceTooHigh { .. } => Self::NonceTooHigh,
            InvalidTransaction::NonceTooLow { tx, state } => Self::NonceTooLow { tx, state },
            InvalidTransaction::AccessListNotSupported => Self::AccessListNotSupported,
            InvalidTransaction::MaxFeePerBlobGasNotSupported => Self::MaxFeePerBlobGasNotSupported,
            InvalidTransaction::BlobVersionedHashesNotSupported => {
                Self::BlobVersionedHashesNotSupported
            }
            InvalidTransaction::BlobGasPriceGreaterThanMax => Self::BlobFeeCapTooLow,
            InvalidTransaction::EmptyBlobs => Self::BlobTransactionMissingBlobHashes,
            InvalidTransaction::BlobVersionNotSupported => Self::BlobHashVersionMismatch,
            InvalidTransaction::TooManyBlobs { have, .. } => Self::TooManyBlobs { have },
            InvalidTransaction::BlobCreateTransaction => Self::BlobTransactionIsCreate,
            InvalidTransaction::AuthorizationListNotSupported => {
                Self::AuthorizationListNotSupported
            }
            InvalidTransaction::AuthorizationListInvalidFields |
            InvalidTransaction::EmptyAuthorizationList => Self::AuthorizationListInvalidFields,
            InvalidTransaction::Eip2930NotSupported |
            InvalidTransaction::Eip1559NotSupported |
            InvalidTransaction::Eip4844NotSupported |
            InvalidTransaction::Eip7702NotSupported |
            InvalidTransaction::Eip7873NotSupported => Self::TxTypeNotSupported,
            InvalidTransaction::Eip7873MissingTarget => {
                Self::other(internal_rpc_err(err.to_string()))
            }
        }
    }
}

impl From<InvalidTransactionError> for RpcInvalidTransactionError {
    fn from(err: InvalidTransactionError) -> Self {
        use InvalidTransactionError;
        // This conversion is used to convert any transaction errors that could occur inside the
        // txpool (e.g. `eth_sendRawTransaction`) to their corresponding RPC
        match err {
            InvalidTransactionError::InsufficientFunds(res) => {
                Self::InsufficientFunds { cost: res.expected, balance: res.got }
            }
            InvalidTransactionError::NonceNotConsistent { tx, state } => {
                Self::NonceTooLow { tx, state }
            }
            InvalidTransactionError::OldLegacyChainId => {
                // Note: this should be unreachable since Spurious Dragon now enabled
                Self::OldLegacyChainId
            }
            InvalidTransactionError::ChainIdMismatch => Self::InvalidChainId,
            InvalidTransactionError::Eip2930Disabled |
            InvalidTransactionError::Eip1559Disabled |
            InvalidTransactionError::Eip4844Disabled |
            InvalidTransactionError::Eip7702Disabled |
            InvalidTransactionError::TxTypeNotSupported => Self::TxTypeNotSupported,
            InvalidTransactionError::GasUintOverflow => Self::GasUintOverflow,
            InvalidTransactionError::GasTooLow => Self::GasTooLow,
            InvalidTransactionError::GasTooHigh => Self::GasTooHigh,
            InvalidTransactionError::TipAboveFeeCap => Self::TipAboveFeeCap,
            InvalidTransactionError::FeeCapTooLow => Self::FeeCapTooLow,
            InvalidTransactionError::SignerAccountHasBytecode => Self::SenderNoEOA,
            InvalidTransactionError::GasLimitTooHigh => Self::GasLimitTooHigh,
        }
    }
}

/// Represents a reverted transaction and its output data.
///
/// Displays "execution reverted(: reason)?" if the reason is a string.
#[derive(Debug, Clone, thiserror::Error)]
pub struct RevertError {
    /// The transaction output data
    ///
    /// Note: this is `None` if output was empty
    output: Option<Bytes>,
}

// === impl RevertError ==

impl RevertError {
    /// Wraps the output bytes
    ///
    /// Note: this is intended to wrap an revm output
    pub fn new(output: Bytes) -> Self {
        if output.is_empty() {
            Self { output: None }
        } else {
            Self { output: Some(output) }
        }
    }

    /// Returns error code to return for this error.
    pub const fn error_code(&self) -> i32 {
        EthRpcErrorCode::ExecutionError.code()
    }
}

impl std::fmt::Display for RevertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("execution reverted")?;
        if let Some(reason) = self.output.as_ref().and_then(|out| RevertReason::decode(out)) {
            let error = reason.to_string();
            let mut error = error.as_str();
            if matches!(reason, RevertReason::ContractError(ContractError::Revert(_))) {
                // we strip redundant `revert: ` prefix from the revert reason
                error = error.trim_start_matches("revert: ");
            }
            write!(f, ": {error}")?;
        }
        Ok(())
    }
}

/// A helper error type that's mainly used to mirror `geth` Txpool's error messages
#[derive(Debug, thiserror::Error)]
pub enum RpcPoolError {
    /// When the transaction is already known
    #[error("already known")]
    AlreadyKnown,
    /// When the sender is invalid
    #[error("invalid sender")]
    InvalidSender,
    /// When the transaction is underpriced
    #[error("transaction underpriced")]
    Underpriced,
    /// When the transaction pool is full
    #[error("txpool is full")]
    TxPoolOverflow,
    /// When the replacement transaction is underpriced
    #[error("replacement transaction underpriced")]
    ReplaceUnderpriced,
    /// When the transaction exceeds the block gas limit
    #[error("exceeds block gas limit")]
    ExceedsGasLimit,
    /// When the transaction gas limit exceeds the maximum transaction gas limit
    #[error("exceeds max transaction gas limit")]
    MaxTxGasLimitExceeded,
    /// Thrown when a new transaction is added to the pool, but then immediately discarded to
    /// respect the tx fee exceeds the configured cap
    #[error("tx fee ({max_tx_fee_wei} wei) exceeds the configured cap ({tx_fee_cap_wei} wei)")]
    ExceedsFeeCap {
        /// max fee in wei of new tx submitted to the pull (e.g. 0.11534 ETH)
        max_tx_fee_wei: u128,
        /// configured tx fee cap in wei (e.g. 1.0 ETH)
        tx_fee_cap_wei: u128,
    },
    /// When a negative value is encountered
    #[error("negative value")]
    NegativeValue,
    /// When oversized data is encountered
    #[error("oversized data")]
    OversizedData,
    /// When the max initcode size is exceeded
    #[error("max initcode size exceeded")]
    ExceedsMaxInitCodeSize,
    /// Errors related to invalid transactions
    #[error(transparent)]
    Invalid(#[from] RpcInvalidTransactionError),
    /// Custom pool error
    #[error(transparent)]
    PoolTransactionError(Box<dyn PoolTransactionError>),
    /// EIP-4844 related error
    #[error(transparent)]
    Eip4844(#[from] Eip4844PoolTransactionError),
    /// EIP-7702 related error
    #[error(transparent)]
    Eip7702(#[from] Eip7702PoolTransactionError),
    /// Thrown if a conflicting transaction type is already in the pool
    ///
    /// In other words, thrown if a transaction with the same sender that violates the exclusivity
    /// constraint (blob vs normal tx)
    #[error("address already reserved")]
    AddressAlreadyReserved,
    /// Other unspecified error
    #[error(transparent)]
    Other(Box<dyn core::error::Error + Send + Sync>),
}

impl From<RpcPoolError> for jsonrpsee_types::error::ErrorObject<'static> {
    fn from(error: RpcPoolError) -> Self {
        match error {
            RpcPoolError::Invalid(err) => err.into(),
            RpcPoolError::TxPoolOverflow => {
                rpc_error_with_code(EthRpcErrorCode::TransactionRejected.code(), error.to_string())
            }
            RpcPoolError::AlreadyKnown |
            RpcPoolError::InvalidSender |
            RpcPoolError::Underpriced |
            RpcPoolError::ReplaceUnderpriced |
            RpcPoolError::ExceedsGasLimit |
            RpcPoolError::MaxTxGasLimitExceeded |
            RpcPoolError::ExceedsFeeCap { .. } |
            RpcPoolError::NegativeValue |
            RpcPoolError::OversizedData |
            RpcPoolError::ExceedsMaxInitCodeSize |
            RpcPoolError::PoolTransactionError(_) |
            RpcPoolError::Eip4844(_) |
            RpcPoolError::Eip7702(_) |
            RpcPoolError::AddressAlreadyReserved => {
                rpc_error_with_code(EthRpcErrorCode::InvalidInput.code(), error.to_string())
            }
            RpcPoolError::Other(other) => internal_rpc_err(other.to_string()),
        }
    }
}

impl From<PoolError> for RpcPoolError {
    fn from(err: PoolError) -> Self {
        match err.kind {
            PoolErrorKind::ReplacementUnderpriced => Self::ReplaceUnderpriced,
            PoolErrorKind::FeeCapBelowMinimumProtocolFeeCap(_) => Self::Underpriced,
            PoolErrorKind::SpammerExceededCapacity(_) | PoolErrorKind::DiscardedOnInsert => {
                Self::TxPoolOverflow
            }
            PoolErrorKind::InvalidTransaction(err) => err.into(),
            PoolErrorKind::Other(err) => Self::Other(err),
            PoolErrorKind::AlreadyImported => Self::AlreadyKnown,
            PoolErrorKind::ExistingConflictingTransactionType(_, _) => Self::AddressAlreadyReserved,
        }
    }
}

impl From<InvalidPoolTransactionError> for RpcPoolError {
    fn from(err: InvalidPoolTransactionError) -> Self {
        match err {
            InvalidPoolTransactionError::Consensus(err) => Self::Invalid(err.into()),
            InvalidPoolTransactionError::ExceedsGasLimit(_, _) => Self::ExceedsGasLimit,
            InvalidPoolTransactionError::MaxTxGasLimitExceeded(_, _) => Self::MaxTxGasLimitExceeded,
            InvalidPoolTransactionError::ExceedsFeeCap { max_tx_fee_wei, tx_fee_cap_wei } => {
                Self::ExceedsFeeCap { max_tx_fee_wei, tx_fee_cap_wei }
            }
            InvalidPoolTransactionError::ExceedsMaxInitCodeSize(_, _) => {
                Self::ExceedsMaxInitCodeSize
            }
            InvalidPoolTransactionError::IntrinsicGasTooLow => {
                Self::Invalid(RpcInvalidTransactionError::GasTooLow)
            }
            InvalidPoolTransactionError::OversizedData(_, _) => Self::OversizedData,
            InvalidPoolTransactionError::Underpriced => Self::Underpriced,
            InvalidPoolTransactionError::Eip2681 => {
                Self::Invalid(RpcInvalidTransactionError::NonceMaxValue)
            }
            InvalidPoolTransactionError::Other(err) => Self::PoolTransactionError(err),
            InvalidPoolTransactionError::Eip4844(err) => Self::Eip4844(err),
            InvalidPoolTransactionError::Eip7702(err) => Self::Eip7702(err),
            InvalidPoolTransactionError::Overdraft { cost, balance } => {
                Self::Invalid(RpcInvalidTransactionError::InsufficientFunds { cost, balance })
            }
            InvalidPoolTransactionError::PriorityFeeBelowMinimum { minimum_priority_fee } => {
                Self::Invalid(RpcInvalidTransactionError::PriorityFeeBelowMinimum {
                    minimum_priority_fee,
                })
            }
        }
    }
}

impl From<PoolError> for EthApiError {
    fn from(err: PoolError) -> Self {
        Self::PoolError(RpcPoolError::from(err))
    }
}

/// Errors returned from a sign request.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    /// Error occurred while trying to sign data.
    #[error("could not sign")]
    CouldNotSign,
    /// Signer for requested account not found.
    #[error("unknown account")]
    NoAccount,
    /// `TypedData` has invalid format.
    #[error("given typed data is not valid")]
    InvalidTypedData,
    /// Invalid transaction request in `sign_transaction`.
    #[error("invalid transaction request")]
    InvalidTransactionRequest,
    /// No chain ID was given.
    #[error("no chainid")]
    NoChainId,
}

/// Converts the evm [`ExecutionResult`] into a result where `Ok` variant is the output bytes if it
/// is [`ExecutionResult::Success`].
pub fn ensure_success<Halt, Error: FromEvmHalt<Halt> + FromEthApiError>(
    result: ExecutionResult<Halt>,
) -> Result<Bytes, Error> {
    match result {
        ExecutionResult::Success { output, .. } => Ok(output.into_data()),
        ExecutionResult::Revert { output, .. } => {
            Err(Error::from_eth_err(RpcInvalidTransactionError::Revert(RevertError::new(output))))
        }
        ExecutionResult::Halt { reason, gas_used } => Err(Error::from_evm_halt(reason, gas_used)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_sol_types::{Revert, SolError};
    use revm::primitives::b256;

    #[test]
    fn timed_out_error() {
        let err = EthApiError::ExecutionTimedOut(Duration::from_secs(10));
        assert_eq!(err.to_string(), "execution aborted (timeout = 10s)");
    }

    #[test]
    fn header_not_found_message() {
        let err: jsonrpsee_types::error::ErrorObject<'static> =
            EthApiError::HeaderNotFound(BlockId::hash(b256!(
                "0x1a15e3c30cf094a99826869517b16d185d45831d3a494f01030b0001a9d3ebb9"
            )))
            .into();
        assert_eq!(
            err.message(),
            "block not found: hash 0x1a15e3c30cf094a99826869517b16d185d45831d3a494f01030b0001a9d3ebb9"
        );
        let err: jsonrpsee_types::error::ErrorObject<'static> =
            EthApiError::HeaderNotFound(BlockId::hash_canonical(b256!(
                "0x1a15e3c30cf094a99826869517b16d185d45831d3a494f01030b0001a9d3ebb9"
            )))
            .into();
        assert_eq!(
            err.message(),
            "block not found: canonical hash 0x1a15e3c30cf094a99826869517b16d185d45831d3a494f01030b0001a9d3ebb9"
        );
        let err: jsonrpsee_types::error::ErrorObject<'static> =
            EthApiError::HeaderNotFound(BlockId::number(100000)).into();
        assert_eq!(err.message(), "block not found: 0x186a0");
        let err: jsonrpsee_types::error::ErrorObject<'static> =
            EthApiError::HeaderNotFound(BlockId::latest()).into();
        assert_eq!(err.message(), "block not found: latest");
        let err: jsonrpsee_types::error::ErrorObject<'static> =
            EthApiError::HeaderNotFound(BlockId::safe()).into();
        assert_eq!(err.message(), "block not found: safe");
        let err: jsonrpsee_types::error::ErrorObject<'static> =
            EthApiError::HeaderNotFound(BlockId::finalized()).into();
        assert_eq!(err.message(), "block not found: finalized");
    }

    #[test]
    fn revert_err_display() {
        let revert = Revert::from("test_revert_reason");
        let err = RevertError::new(revert.abi_encode().into());
        let msg = err.to_string();
        assert_eq!(msg, "execution reverted: test_revert_reason");
    }
}
