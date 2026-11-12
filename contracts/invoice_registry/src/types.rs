use soroban_sdk::{contracttype, Address, BytesN, String};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RiskTier {
    /// Prime corporate debtor (lowest risk, lowest discount rate)
    TierA = 0,
    /// Mid-market corporate debtor (standard risk)
    TierB = 1,
    /// Emerging/SME debtor (higher risk, higher yield)
    TierC = 2,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvoiceStatus {
    /// Newly minted by a merchant, awaiting configured verifier/admin approval
    Issued = 0,
    /// Approved by the configured verifier or admin, eligible for factoring
    Verified = 1,
    /// Factored via FactoringEscrow, ownership locked in escrow
    Factored = 2,
    /// Full face value repaid at maturity
    Settled = 3,
    /// Past grace period without repayment
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryConfig {
    pub admin: Address,
    pub verifier: Address,
    pub factoring_escrow: Address,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    NextInvoiceId,
    Invoice(u64),
    MerchantInvoices(Address),
}
