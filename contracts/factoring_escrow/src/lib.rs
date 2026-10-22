#![no_std]

pub mod errors;
pub mod events;
pub mod types;

#[cfg(test)]
mod test;

use crate::errors::EscrowError;
use crate::types::{
    DataKey, EscrowConfig, EscrowRecord, EscrowStatus, FactoringQuote, RateConfig,
};
use invoice_registry::types::{InvoiceStatus, RiskTier};
use invoice_registry::InvoiceRegistryContractClient;
use liquidity_pool::LiquidityPoolContractClient;
use soroban_sdk::{contract, contractimpl, Address, Env};

const BPS_DENOMINATOR: i128 = 10_000;
const SECONDS_PER_YEAR: i128 = 31_536_000;
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17_280;
const INSTANCE_BUMP_AMOUNT: u32 = 518_400;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = 34_560;
const PERSISTENT_BUMP_AMOUNT: u32 = 1_036_800;

#[contract]
pub struct FactoringEscrowContract;

#[contractimpl]
impl FactoringEscrowContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        invoice_registry: Address,
        liquidity_pool: Address,
        usdc_token: Address,
        rate_config: RateConfig,
    ) -> Result<(), EscrowError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(EscrowError::AlreadyInitialized);
        }
        admin.require_auth();
        Self::validate_rate_config(&rate_config)?;

        let config = EscrowConfig {
            admin: admin.clone(),
            invoice_registry: invoice_registry.clone(),
            liquidity_pool: liquidity_pool.clone(),
            usdc_token,
            rate_config,
            paused: false,
        };
        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::TotalFactoredVolume, &0i128);
        env.storage().instance().set(&DataKey::ActiveEscrowCount, &0u64);
        Self::extend_instance_ttl(&env);

        events::emit_escrow_initialized(&env, &admin, &invoice_registry, &liquidity_pool);
        Ok(())
    }

    pub fn calculate_discount(
        env: Env,
        invoice_id: u64,
        face_value: i128,
        risk_tier: RiskTier,
        maturity_timestamp: u64,
    ) -> Result<FactoringQuote, EscrowError> {
        let config = Self::load_config(&env)?;
        let now = env.ledger().timestamp();
        Self::compute_quote(invoice_id, face_value, risk_tier, now, maturity_timestamp, &config.rate_config)
    }

    pub fn preview_factoring_quote(env: Env, invoice_id: u64) -> Result<FactoringQuote, EscrowError> {
        let config = Self::load_config(&env)?;
        let registry_client = InvoiceRegistryContractClient::new(&env, &config.invoice_registry);
        let invoice = registry_client.get_invoice(&invoice_id);
        let now = env.ledger().timestamp();
        Self::compute_quote(invoice.invoice_id, invoice.face_value, invoice.risk_tier, now, invoice.maturity_timestamp, &config.rate_config)
    }

    /// Executes an atomic swap of a verified unpaid trade invoice for immediate USDC liquidity.
    /// Strictly implements the Checks-Effects-Interactions (CEI) pattern.
    pub fn factor_invoice(env: Env, merchant: Address, invoice_id: u64) -> Result<FactoringQuote, EscrowError> {
        merchant.require_auth();
        let config = Self::load_config(&env)?;
        if config.paused {
            return Err(EscrowError::ContractPaused);
        }

        let registry_client = InvoiceRegistryContractClient::new(&env, &config.invoice_registry);
        let invoice = registry_client.get_invoice(&invoice_id);

        if invoice.owner != merchant {
            return Err(EscrowError::NotInvoiceOwner);
        }
        if invoice.status != InvoiceStatus::Verified {
            return Err(EscrowError::InvoiceNotVerified);
        }
        if invoice.currency != config.usdc_token {
            return Err(EscrowError::CurrencyMismatch);
        }

        let now = env.ledger().timestamp();
        let quote = Self::compute_quote(
            invoice_id,
            invoice.face_value,
            invoice.risk_tier,
            now,
            invoice.maturity_timestamp,
            &config.rate_config,
        )?;

        let escrow_record = EscrowRecord {
            invoice_id,
            merchant: merchant.clone(),
            debtor: invoice.debtor.clone(),
            face_value: invoice.face_value,
            net_payout: quote.net_payout,
            total_discount: quote.total_discount,
            risk_tier: invoice.risk_tier,
            factored_at: now,
            maturity_timestamp: invoice.maturity_timestamp,
            settled_at: 0,
            status: EscrowStatus::Active,
        };

        let escrow_key = DataKey::Escrow(invoice_id);
        env.storage().persistent().set(&escrow_key, &escrow_record);
        Self::extend_persistent_ttl(&env, &escrow_key);

        let total_volume: i128 = env.storage().instance().get(&DataKey::TotalFactoredVolume).unwrap_or(0);
        let new_volume = total_volume.checked_add(invoice.face_value).ok_or(EscrowError::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::TotalFactoredVolume, &new_volume);

        let active_count: u64 = env.storage().instance().get(&DataKey::ActiveEscrowCount).unwrap_or(0);
        let new_active = active_count.checked_add(1).ok_or(EscrowError::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::ActiveEscrowCount, &new_active);
        Self::extend_instance_ttl(&env);

        let escrow_addr = env.current_contract_address();
        registry_client.mark_factored(&escrow_addr, &invoice_id);

        let pool_client = LiquidityPoolContractClient::new(&env, &config.liquidity_pool);
        pool_client.allocate_liquidity(&escrow_addr, &merchant, &quote.net_payout);

        events::emit_invoice_factored(&env, invoice_id, &merchant, &quote);
        Ok(quote)
    }

    pub fn settle_invoice(env: Env, payer: Address, invoice_id: u64) -> Result<(i128, i128), EscrowError> {
        payer.require_auth();
        let config = Self::load_config(&env)?;
        if config.paused {
            return Err(EscrowError::ContractPaused);
        }

        let mut escrow_record = Self::load_escrow(&env, invoice_id)?;
        if escrow_record.status != EscrowStatus::Active {
            return Err(EscrowError::EscrowAlreadySettled);
        }

        escrow_record.status = EscrowStatus::Settled;
        escrow_record.settled_at = env.ledger().timestamp();
        let escrow_key = DataKey::Escrow(invoice_id);
        env.storage().persistent().set(&escrow_key, &escrow_record);
        Self::extend_persistent_ttl(&env, &escrow_key);

        let active_count: u64 = env.storage().instance().get(&DataKey::ActiveEscrowCount).unwrap_or(1);
        env.storage().instance().set(&DataKey::ActiveEscrowCount, &active_count.saturating_sub(1));
        Self::extend_instance_ttl(&env);

        let escrow_addr = env.current_contract_address();
        let pool_client = LiquidityPoolContractClient::new(&env, &config.liquidity_pool);
        let (lp_yield, protocol_cut) = pool_client.repay_liquidity(
            &escrow_addr,
            &payer,
            &escrow_record.net_payout,
            &escrow_record.total_discount,
        );

        let registry_client = InvoiceRegistryContractClient::new(&env, &config.invoice_registry);
        registry_client.mark_settled(&escrow_addr, &invoice_id);

        events::emit_invoice_settled(&env, invoice_id, &payer, escrow_record.face_value, lp_yield, protocol_cut);
        Ok((lp_yield, protocol_cut))
    }

    pub fn mark_defaulted(env: Env, caller: Address, invoice_id: u64) -> Result<(), EscrowError> {
        caller.require_auth();
        let config = Self::load_config(&env)?;
        if caller != config.admin {
            return Err(EscrowError::Unauthorized);
        }

        let mut escrow_record = Self::load_escrow(&env, invoice_id)?;
        if escrow_record.status != EscrowStatus::Active {
            return Err(EscrowError::EscrowAlreadySettled);
        }

        let deadline = escrow_record.maturity_timestamp.checked_add(config.rate_config.grace_period_seconds).ok_or(EscrowError::ArithmeticOverflow)?;
        if env.ledger().timestamp() <= deadline {
            return Err(EscrowError::GracePeriodNotElapsed);
        }

        escrow_record.status = EscrowStatus::Defaulted;
        let escrow_key = DataKey::Escrow(invoice_id);
        env.storage().persistent().set(&escrow_key, &escrow_record);
        Self::extend_persistent_ttl(&env, &escrow_key);

        let active_count: u64 = env.storage().instance().get(&DataKey::ActiveEscrowCount).unwrap_or(1);
        env.storage().instance().set(&DataKey::ActiveEscrowCount, &active_count.saturating_sub(1));

        let escrow_addr = env.current_contract_address();
        let pool_client = LiquidityPoolContractClient::new(&env, &config.liquidity_pool);
        pool_client.write_off_default(&escrow_addr, &escrow_record.net_payout);

        let registry_client = InvoiceRegistryContractClient::new(&env, &config.invoice_registry);
        registry_client.mark_defaulted(&escrow_addr, &invoice_id);

        events::emit_invoice_defaulted(&env, invoice_id, escrow_record.net_payout);
        Ok(())
    }

    /// Admin-only circuit breaker. While paused, `factor_invoice` and `settle_invoice` are rejected.
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), EscrowError> {
        let mut config = Self::require_admin(&env, &admin)?;
        config.paused = paused;
        env.storage().instance().set(&DataKey::Config, &config);
        Self::extend_instance_ttl(&env);
        events::emit_paused_changed(&env, &admin, paused);
        Ok(())
    }

    /// Admin-only update of the discount rate configuration. Applies to future quotes only.
    pub fn update_rate_config(env: Env, admin: Address, rate_config: RateConfig) -> Result<(), EscrowError> {
        let mut config = Self::require_admin(&env, &admin)?;
        Self::validate_rate_config(&rate_config)?;
        config.rate_config = rate_config;
        env.storage().instance().set(&DataKey::Config, &config);
        Self::extend_instance_ttl(&env);
        events::emit_rate_config_updated(&env, &admin);
        Ok(())
    }

    /// Admin-only transfer of the admin role. The new admin must also authorize the transfer.
    pub fn transfer_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), EscrowError> {
        let mut config = Self::require_admin(&env, &admin)?;
        new_admin.require_auth();
        config.admin = new_admin.clone();
        env.storage().instance().set(&DataKey::Config, &config);
        Self::extend_instance_ttl(&env);
        events::emit_admin_transferred(&env, &admin, &new_admin);
        Ok(())
    }

    pub fn get_escrow_record(env: Env, invoice_id: u64) -> Result<EscrowRecord, EscrowError> {
        Self::load_escrow(&env, invoice_id)
    }

    pub fn get_protocol_metrics(env: Env) -> (i128, u64) {
        let total_volume: i128 = env.storage().instance().get(&DataKey::TotalFactoredVolume).unwrap_or(0);
        let active_count: u64 = env.storage().instance().get(&DataKey::ActiveEscrowCount).unwrap_or(0);
        (total_volume, active_count)
    }

    fn compute_quote(
        invoice_id: u64,
        face_value: i128,
        risk_tier: RiskTier,
        now: u64,
        maturity_timestamp: u64,
        rate_config: &RateConfig,
    ) -> Result<FactoringQuote, EscrowError> {
        if maturity_timestamp <= now {
            return Err(EscrowError::InvoiceAlreadyExpired);
        }
        let duration_seconds = maturity_timestamp.checked_sub(now).ok_or(EscrowError::ArithmeticOverflow)?;

        let annual_bps = match risk_tier {
            RiskTier::TierA => rate_config.tier_a_annual_bps as i128,
            RiskTier::TierB => rate_config.tier_b_annual_bps as i128,
            RiskTier::TierC => rate_config.tier_c_annual_bps as i128,
        };

        let time_discount_numerator = face_value
            .checked_mul(annual_bps)
            .ok_or(EscrowError::ArithmeticOverflow)?
            .checked_mul(duration_seconds as i128)
            .ok_or(EscrowError::ArithmeticOverflow)?;
        let time_discount_denominator = BPS_DENOMINATOR
            .checked_mul(SECONDS_PER_YEAR)
            .ok_or(EscrowError::ArithmeticOverflow)?;
        let time_discount_amount = time_discount_numerator
            .checked_div(time_discount_denominator)
            .ok_or(EscrowError::ArithmeticOverflow)?;

        let origination_fee_amount = face_value
            .checked_mul(rate_config.origination_fee_bps as i128)
            .ok_or(EscrowError::ArithmeticOverflow)?
            .checked_div(BPS_DENOMINATOR)
            .ok_or(EscrowError::ArithmeticOverflow)?;

        let total_discount = time_discount_amount
            .checked_add(origination_fee_amount)
            .ok_or(EscrowError::ArithmeticOverflow)?;
        if total_discount >= face_value {
            return Err(EscrowError::DiscountExceedsFaceValue);
        }

        let net_payout = face_value
            .checked_sub(total_discount)
            .ok_or(EscrowError::ArithmeticOverflow)?;

        let effective_discount_bps = total_discount
            .checked_mul(BPS_DENOMINATOR)
            .ok_or(EscrowError::ArithmeticOverflow)?
            .checked_div(face_value)
            .ok_or(EscrowError::ArithmeticOverflow)? as u32;

        Ok(FactoringQuote {
            invoice_id,
            face_value,
            risk_tier,
            duration_seconds,
            time_discount_amount,
            origination_fee_amount,
            total_discount,
            net_payout,
            effective_discount_bps,
        })
    }

    fn validate_rate_config(cfg: &RateConfig) -> Result<(), EscrowError> {
        if cfg.tier_a_annual_bps == 0
            || cfg.tier_a_annual_bps > cfg.tier_b_annual_bps
            || cfg.tier_b_annual_bps > cfg.tier_c_annual_bps
            || cfg.tier_c_annual_bps > 5_000
            || cfg.origination_fee_bps > 1_000
        {
            return Err(EscrowError::InvalidRateConfiguration);
        }
        Ok(())
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<EscrowConfig, EscrowError> {
        caller.require_auth();
        let config = Self::load_config(env)?;
        if *caller != config.admin {
            return Err(EscrowError::Unauthorized);
        }
        Ok(config)
    }

    fn load_config(env: &Env) -> Result<EscrowConfig, EscrowError> {
        env.storage().instance().get(&DataKey::Config).ok_or(EscrowError::NotInitialized)
    }

    fn load_escrow(env: &Env, invoice_id: u64) -> Result<EscrowRecord, EscrowError> {
        let key = DataKey::Escrow(invoice_id);
        let record = env.storage().persistent().get(&key).ok_or(EscrowError::EscrowNotFound)?;
        Self::extend_persistent_ttl(env, &key);
        Ok(record)
    }

    fn extend_instance_ttl(env: &Env) {
        env.storage().instance().extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }

    fn extend_persistent_ttl(env: &Env, key: &DataKey) {
        env.storage().persistent().extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
    }
}
