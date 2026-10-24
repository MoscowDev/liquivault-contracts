//! Cross-contract interfaces for `invoice_registry` and `liquidity_pool`.
//!
//! The escrow must not link the other contract crates into its WASM binary: their exported
//! entrypoints (`initialize`, `set_paused`, ...) would collide with the escrow's own. Instead we
//! declare only the functions the escrow calls, plus wire-compatible copies of the shared types.
//! These definitions MUST stay field- and discriminant-identical to `invoice_registry::types`.

use soroban_sdk::{contractclient, contracttype, Address, BytesN, Env, String};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RiskTier {
    TierA = 0,
    TierB = 1,
    TierC = 2,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvoiceStatus {
    Issued = 0,
    Verified = 1,
    Factored = 2,
    Settled = 3,
    Defaulted = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceRecord {
    pub invoice_id: u64,
    pub merchant: Address,
    pub owner: Address,
    pub debtor: Address,
    pub currency: Address,
    pub face_value: i128,
    pub issued_at: u64,
    pub maturity_timestamp: u64,
    pub metadata_hash: BytesN<32>,
    pub invoice_number: String,
    pub risk_tier: RiskTier,
    pub status: InvoiceStatus,
}

#[contractclient(name = "InvoiceRegistryClient")]
pub trait InvoiceRegistryInterface {
    fn get_invoice(env: Env, invoice_id: u64) -> InvoiceRecord;
    fn mark_factored(env: Env, escrow_caller: Address, invoice_id: u64);
    fn mark_settled(env: Env, escrow_caller: Address, invoice_id: u64);
    fn mark_defaulted(env: Env, caller: Address, invoice_id: u64);
}

#[contractclient(name = "LiquidityPoolClient")]
pub trait LiquidityPoolInterface {
    fn allocate_liquidity(
        env: Env,
        escrow_caller: Address,
        merchant_recipient: Address,
        principal_amount: i128,
    );
    fn repay_liquidity(
        env: Env,
        escrow_caller: Address,
        payer: Address,
        principal_repaid: i128,
        gross_yield: i128,
    ) -> (i128, i128);
    fn write_off_default(env: Env, escrow_caller: Address, defaulted_principal: i128);
}
