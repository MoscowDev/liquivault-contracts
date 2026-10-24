use crate::interfaces::RiskTier;
use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowStatus {
    Active = 0,
    Settled = 1,
    Defaulted = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateConfig {
    pub tier_a_annual_bps: u32,
    pub tier_b_annual_bps: u32,
    pub tier_c_annual_bps: u32,
    pub origination_fee_bps: u32,
    pub grace_period_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowConfig {
    pub admin: Address,
    pub invoice_registry: Address,
    pub liquidity_pool: Address,
    pub usdc_token: Address,
    pub rate_config: RateConfig,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoringQuote {
    pub invoice_id: u64,
    pub face_value: i128,
    pub risk_tier: RiskTier,
    pub duration_seconds: u64,
    pub time_discount_amount: i128,
    pub origination_fee_amount: i128,
    pub total_discount: i128,
    pub net_payout: i128,
    pub effective_discount_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRecord {
    pub invoice_id: u64,
    pub merchant: Address,
    pub debtor: Address,
    pub face_value: i128,
    pub net_payout: i128,
    pub total_discount: i128,
    pub risk_tier: RiskTier,
    pub factored_at: u64,
    pub maturity_timestamp: u64,
    pub settled_at: u64,
    pub status: EscrowStatus,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    Escrow(u64),
    TotalFactoredVolume,
    ActiveEscrowCount,
}
