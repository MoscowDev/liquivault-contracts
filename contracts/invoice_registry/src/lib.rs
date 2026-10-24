#![no_std]

pub mod errors;
pub mod events;
pub mod types;

#[cfg(test)]
mod test;

use crate::errors::RegistryError;
use crate::types::{DataKey, InvoiceRecord, InvoiceStatus, RegistryConfig, RiskTier};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Vec};

const INSTANCE_LIFETIME_THRESHOLD: u32 = 17_280; // ~1 day
const INSTANCE_BUMP_AMOUNT: u32 = 518_400; // ~30 days
const PERSISTENT_LIFETIME_THRESHOLD: u32 = 34_560; // ~2 days
const PERSISTENT_BUMP_AMOUNT: u32 = 1_036_800; // ~60 days

#[contract]
pub struct InvoiceRegistryContract;

#[contractimpl]
impl InvoiceRegistryContract {
    /// Initializes the InvoiceRegistry contract with admin, verifier oracle, and authorized factoring escrow.
    pub fn initialize(
        env: Env,
        admin: Address,
        verifier: Address,
        factoring_escrow: Address,
    ) -> Result<(), RegistryError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(RegistryError::AlreadyInitialized);
        }
        admin.require_auth();

        let config = RegistryConfig {
            admin: admin.clone(),
            verifier: verifier.clone(),
            factoring_escrow: factoring_escrow.clone(),
            paused: false,
        };

        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::NextInvoiceId, &1u64);
        Self::extend_instance_ttl(&env);

        events::emit_initialized(&env, &admin, &verifier, &factoring_escrow);
        Ok(())
    }

    /// Updates the authorized FactoringEscrow contract address (Admin only).
    pub fn set_factoring_escrow(
        env: Env,
        caller: Address,
        factoring_escrow: Address,
    ) -> Result<(), RegistryError> {
        caller.require_auth();
        let mut config = Self::load_config(&env)?;
        if caller != config.admin {
            return Err(RegistryError::Unauthorized);
        }
        config.factoring_escrow = factoring_escrow;
        env.storage().instance().set(&DataKey::Config, &config);
        Self::extend_instance_ttl(&env);
        Ok(())
    }

    /// Pauses or unpauses invoice registry operations (Admin only).
    pub fn set_paused(env: Env, caller: Address, paused: bool) -> Result<(), RegistryError> {
        caller.require_auth();
        let mut config = Self::load_config(&env)?;
        if caller != config.admin {
            return Err(RegistryError::Unauthorized);
        }
        config.paused = paused;
        env.storage().instance().set(&DataKey::Config, &config);
        Self::extend_instance_ttl(&env);
        Ok(())
    }

    /// Tokenizes a new trade receivable on-chain with SHA-256 metadata commitment.
    /// Follows Checks-Effects-Interactions (CEI) and checked arithmetic.
    pub fn mint_invoice(
        env: Env,
        merchant: Address,
        debtor: Address,
        currency: Address,
        face_value: i128,
        maturity_timestamp: u64,
        metadata_hash: BytesN<32>,
        invoice_number: String,
        risk_tier: RiskTier,
    ) -> Result<u64, RegistryError> {
        // 1. CHECKS
        merchant.require_auth();
        let config = Self::load_config(&env)?;
        if config.paused {
            return Err(RegistryError::ContractPaused);
        }
        if merchant == debtor {
            return Err(RegistryError::SameParties);
        }
        if face_value <= 0 {
            return Err(RegistryError::InvalidFaceValue);
        }
        let now = env.ledger().timestamp();
        if maturity_timestamp <= now {
            return Err(RegistryError::InvalidMaturityTimestamp);
        }

        let invoice_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextInvoiceId)
            .unwrap_or(1u64);
        let next_id = invoice_id
            .checked_add(1)
            .ok_or(RegistryError::ArithmeticOverflow)?;

        // 2. EFFECTS
        let record = InvoiceRecord {
            invoice_id,
            merchant: merchant.clone(),
            owner: merchant.clone(),
            debtor: debtor.clone(),
            currency,
            face_value,
            issued_at: now,
            maturity_timestamp,
            metadata_hash: metadata_hash.clone(),
            invoice_number,
            risk_tier,
            status: InvoiceStatus::Issued,
        };

        let inv_key = DataKey::Invoice(invoice_id);
        env.storage().persistent().set(&inv_key, &record);
        Self::extend_persistent_ttl(&env, &inv_key);

        let merchant_key = DataKey::MerchantInvoices(merchant.clone());
        let mut merchant_list: Vec<u64> = env
            .storage()
            .persistent()
            .get(&merchant_key)
            .unwrap_or_else(|| Vec::new(&env));
        merchant_list.push_back(invoice_id);
        env.storage()
            .persistent()
            .set(&merchant_key, &merchant_list);
        Self::extend_persistent_ttl(&env, &merchant_key);

        env.storage()
            .instance()
            .set(&DataKey::NextInvoiceId, &next_id);
        Self::extend_instance_ttl(&env);

        // 3. INTERACTIONS (Events)
        events::emit_invoice_minted(
            &env,
            invoice_id,
            &merchant,
            &debtor,
            face_value,
            maturity_timestamp,
            &metadata_hash,
        );

        Ok(invoice_id)
    }

    /// Verifies an issued invoice and confirms or updates its credit risk tier.
    /// Can be invoked only by the authorized verifier oracle or the admin.
    pub fn verify_invoice(
        env: Env,
        caller: Address,
        invoice_id: u64,
        risk_tier: RiskTier,
    ) -> Result<(), RegistryError> {
        // 1. CHECKS
        caller.require_auth();
        let config = Self::load_config(&env)?;
        if config.paused {
            return Err(RegistryError::ContractPaused);
        }

        let mut record = Self::load_invoice(&env, invoice_id)?;
        // The debtor is intentionally NOT an authorized verifier: the merchant picks the debtor
        // address at mint time, so debtor self-verification would let a merchant approve fake invoices.
        if caller != config.verifier && caller != config.admin {
            return Err(RegistryError::Unauthorized);
        }
        if record.status != InvoiceStatus::Issued {
            return Err(RegistryError::InvalidStatusTransition);
        }

        // 2. EFFECTS
        record.risk_tier = risk_tier;
        record.status = InvoiceStatus::Verified;
        let inv_key = DataKey::Invoice(invoice_id);
        env.storage().persistent().set(&inv_key, &record);
        Self::extend_persistent_ttl(&env, &inv_key);
        Self::extend_instance_ttl(&env);

        // 3. INTERACTIONS (Events)
        events::emit_invoice_verified(&env, invoice_id, &caller, risk_tier);
        Ok(())
    }

    /// Marks a verified invoice as Factored and transfers ownership to the FactoringEscrow contract.
    /// Restricted exclusively to the configured FactoringEscrow contract.
    pub fn mark_factored(
        env: Env,
        escrow_caller: Address,
        invoice_id: u64,
    ) -> Result<(), RegistryError> {
        // 1. CHECKS
        escrow_caller.require_auth();
        let config = Self::load_config(&env)?;
        if config.paused {
            return Err(RegistryError::ContractPaused);
        }
        if escrow_caller != config.factoring_escrow {
            return Err(RegistryError::Unauthorized);
        }

        let mut record = Self::load_invoice(&env, invoice_id)?;
        if record.status != InvoiceStatus::Verified {
            return Err(RegistryError::InvalidStatusTransition);
        }

        // 2. EFFECTS
        let previous_owner = record.owner.clone();
        record.owner = escrow_caller.clone();
        record.status = InvoiceStatus::Factored;

        let inv_key = DataKey::Invoice(invoice_id);
        env.storage().persistent().set(&inv_key, &record);
        Self::extend_persistent_ttl(&env, &inv_key);
        Self::extend_instance_ttl(&env);

        // 3. INTERACTIONS (Events)
        events::emit_ownership_transferred(&env, invoice_id, &previous_owner, &escrow_caller);
        events::emit_status_updated(
            &env,
            invoice_id,
            InvoiceStatus::Verified,
            InvoiceStatus::Factored,
        );
        Ok(())
    }

    /// Marks a Factored invoice as Settled upon maturity repayment.
    /// Restricted exclusively to the configured FactoringEscrow contract.
    pub fn mark_settled(
        env: Env,
        escrow_caller: Address,
        invoice_id: u64,
    ) -> Result<(), RegistryError> {
        escrow_caller.require_auth();
        let config = Self::load_config(&env)?;
        if escrow_caller != config.factoring_escrow {
            return Err(RegistryError::Unauthorized);
        }

        let mut record = Self::load_invoice(&env, invoice_id)?;
        if record.status != InvoiceStatus::Factored {
            return Err(RegistryError::InvalidStatusTransition);
        }

        record.status = InvoiceStatus::Settled;
        let inv_key = DataKey::Invoice(invoice_id);
        env.storage().persistent().set(&inv_key, &record);
        Self::extend_persistent_ttl(&env, &inv_key);

        events::emit_status_updated(
            &env,
            invoice_id,
            InvoiceStatus::Factored,
            InvoiceStatus::Settled,
        );
        Ok(())
    }

    /// Marks a Factored invoice as Defaulted if unpaid past maturity + grace period.
    /// Restricted exclusively to the configured FactoringEscrow contract or Admin.
    pub fn mark_defaulted(env: Env, caller: Address, invoice_id: u64) -> Result<(), RegistryError> {
        caller.require_auth();
        let config = Self::load_config(&env)?;
        if caller != config.factoring_escrow && caller != config.admin {
            return Err(RegistryError::Unauthorized);
        }

        let mut record = Self::load_invoice(&env, invoice_id)?;
        if record.status != InvoiceStatus::Factored {
            return Err(RegistryError::InvalidStatusTransition);
        }

        record.status = InvoiceStatus::Defaulted;
        let inv_key = DataKey::Invoice(invoice_id);
        env.storage().persistent().set(&inv_key, &record);
        Self::extend_persistent_ttl(&env, &inv_key);

        events::emit_status_updated(
            &env,
            invoice_id,
            InvoiceStatus::Factored,
            InvoiceStatus::Defaulted,
        );
        Ok(())
    }

    /// Transfers ownership of an unfactored invoice (Issued or Verified) to a new holder.
    pub fn transfer_ownership(
        env: Env,
        current_owner: Address,
        new_owner: Address,
        invoice_id: u64,
    ) -> Result<(), RegistryError> {
        current_owner.require_auth();
        let config = Self::load_config(&env)?;
        if config.paused {
            return Err(RegistryError::ContractPaused);
        }

        let mut record = Self::load_invoice(&env, invoice_id)?;
        if record.owner != current_owner {
            return Err(RegistryError::Unauthorized);
        }
        if record.status == InvoiceStatus::Factored
            || record.status == InvoiceStatus::Settled
            || record.status == InvoiceStatus::Defaulted
        {
            return Err(RegistryError::InvoiceAlreadyFactored);
        }

        record.owner = new_owner.clone();
        let inv_key = DataKey::Invoice(invoice_id);
        env.storage().persistent().set(&inv_key, &record);
        Self::extend_persistent_ttl(&env, &inv_key);

        events::emit_ownership_transferred(&env, invoice_id, &current_owner, &new_owner);
        Ok(())
    }

    /// Verifies whether `candidate` is the current owner of `invoice_id`.
    pub fn verify_ownership(
        env: Env,
        invoice_id: u64,
        candidate: Address,
    ) -> Result<bool, RegistryError> {
        let record = Self::load_invoice(&env, invoice_id)?;
        Ok(record.owner == candidate)
    }

    /// Verifies whether `candidate_hash` matches the anchored SHA-256 metadata hash of `invoice_id`.
    pub fn verify_metadata_hash(
        env: Env,
        invoice_id: u64,
        candidate_hash: BytesN<32>,
    ) -> Result<bool, RegistryError> {
        let record = Self::load_invoice(&env, invoice_id)?;
        Ok(record.metadata_hash == candidate_hash)
    }

    /// Retrieves the full on-chain record for `invoice_id`.
    pub fn get_invoice(env: Env, invoice_id: u64) -> Result<InvoiceRecord, RegistryError> {
        Self::load_invoice(&env, invoice_id)
    }

    /// Lists all invoice IDs minted by `merchant`.
    pub fn get_merchant_invoices(env: Env, merchant: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::MerchantInvoices(merchant))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the current RegistryConfig.
    pub fn get_config(env: Env) -> Result<RegistryConfig, RegistryError> {
        Self::load_config(&env)
    }

    // --- Internal Helpers ---

    fn load_config(env: &Env) -> Result<RegistryConfig, RegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(RegistryError::NotInitialized)
    }

    fn load_invoice(env: &Env, invoice_id: u64) -> Result<InvoiceRecord, RegistryError> {
        let key = DataKey::Invoice(invoice_id);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::InvoiceNotFound)?;
        Self::extend_persistent_ttl(env, &key);
        Ok(record)
    }

    fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }

    fn extend_persistent_ttl(env: &Env, key: &DataKey) {
        env.storage().persistent().extend_ttl(
            key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }
}
