#![no_std]

pub mod errors;
pub mod events;
pub mod types;

#[cfg(test)]
mod test;

use crate::errors::PoolError;
use crate::types::{DataKey, LpPosition, PoolState};
use soroban_sdk::{contract, contractimpl, token, Address, Env};

const BPS_DENOMINATOR: i128 = 10_000;
const SHARE_PRICE_SCALE: i128 = 10_000_000;
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17_280;
const INSTANCE_BUMP_AMOUNT: u32 = 518_400;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = 34_560;
const PERSISTENT_BUMP_AMOUNT: u32 = 1_036_800;

#[contract]
pub struct LiquidityPoolContract;

#[contractimpl]
impl LiquidityPoolContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        usdc_token: Address,
        factoring_escrow: Address,
        treasury: Address,
        reserve_factor_bps: u32,
    ) -> Result<(), PoolError> {
        if env.storage().instance().has(&DataKey::State) {
            return Err(PoolError::AlreadyInitialized);
        }
        admin.require_auth();
        if reserve_factor_bps > 5_000 {
            return Err(PoolError::InvalidReserveFactor);
        }
        let state = PoolState {
            admin: admin.clone(),
            usdc_token: usdc_token.clone(),
            factoring_escrow,
            treasury,
            total_assets: 0,
            total_shares: 0,
            deployed_capital: 0,
            cumulative_yield: 0,
            reserve_factor_bps,
            paused: false,
        };
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);
        events::emit_pool_initialized(&env, &admin, &usdc_token);
        Ok(())
    }

    pub fn set_factoring_escrow(env: Env, caller: Address, factoring_escrow: Address) -> Result<(), PoolError> {
        caller.require_auth();
        let mut state = Self::load_state(&env)?;
        if caller != state.admin {
            return Err(PoolError::Unauthorized);
        }
        state.factoring_escrow = factoring_escrow;
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);
        Ok(())
    }

    /// Deposits USDC and mints pro-rata LP shares following CEI pattern.
    pub fn deposit(env: Env, provider: Address, amount: i128) -> Result<i128, PoolError> {
        provider.require_auth();
        if amount <= 0 {
            return Err(PoolError::InvalidAmount);
        }
        let mut state = Self::load_state(&env)?;
        if state.paused {
            return Err(PoolError::PoolPaused);
        }
        let shares_to_mint = Self::compute_shares_for_deposit(amount, state.total_assets, state.total_shares)?;
        if shares_to_mint <= 0 {
            return Err(PoolError::ZeroSharesMinted);
        }

        state.total_assets = state.total_assets.checked_add(amount).ok_or(PoolError::ArithmeticOverflow)?;
        state.total_shares = state.total_shares.checked_add(shares_to_mint).ok_or(PoolError::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);

        let pos_key = DataKey::Position(provider.clone());
        let mut position = env.storage().persistent().get(&pos_key).unwrap_or(LpPosition {
            provider: provider.clone(),
            shares: 0,
            deposited_usdc: 0,
            last_deposit_timestamp: 0,
        });
        position.shares = position.shares.checked_add(shares_to_mint).ok_or(PoolError::ArithmeticOverflow)?;
        position.deposited_usdc = position.deposited_usdc.checked_add(amount).ok_or(PoolError::ArithmeticOverflow)?;
        position.last_deposit_timestamp = env.ledger().timestamp();
        env.storage().persistent().set(&pos_key, &position);
        Self::extend_persistent_ttl(&env, &pos_key);

        let token_client = token::Client::new(&env, &state.usdc_token);
        token_client.transfer(&provider, &env.current_contract_address(), &amount);
        events::emit_deposited(&env, &provider, amount, shares_to_mint);
        Ok(shares_to_mint)
    }

    /// Burns LP shares and withdraws underlying USDC + pro-rata yield following CEI pattern.
    pub fn withdraw(env: Env, provider: Address, shares_to_burn: i128) -> Result<i128, PoolError> {
        provider.require_auth();
        if shares_to_burn <= 0 {
            return Err(PoolError::InvalidAmount);
        }
        let mut state = Self::load_state(&env)?;
        if state.paused {
            return Err(PoolError::PoolPaused);
        }
        let pos_key = DataKey::Position(provider.clone());
        let mut position: LpPosition = env.storage().persistent().get(&pos_key).ok_or(PoolError::InsufficientShares)?;
        if position.shares < shares_to_burn {
            return Err(PoolError::InsufficientShares);
        }
        let usdc_to_return = Self::compute_assets_for_shares(shares_to_burn, state.total_assets, state.total_shares)?;
        if usdc_to_return <= 0 {
            return Err(PoolError::ZeroAssetsReturned);
        }
        let idle_liquidity = state.total_assets.checked_sub(state.deployed_capital).ok_or(PoolError::ArithmeticOverflow)?;
        if usdc_to_return > idle_liquidity {
            return Err(PoolError::InsufficientPoolLiquidity);
        }

        let deposit_reduction = position.deposited_usdc.checked_mul(shares_to_burn).ok_or(PoolError::ArithmeticOverflow)?.checked_div(position.shares).ok_or(PoolError::ArithmeticOverflow)?;
        position.shares = position.shares.checked_sub(shares_to_burn).ok_or(PoolError::ArithmeticOverflow)?;
        position.deposited_usdc = position.deposited_usdc.checked_sub(deposit_reduction).unwrap_or(0);
        env.storage().persistent().set(&pos_key, &position);
        Self::extend_persistent_ttl(&env, &pos_key);

        state.total_shares = state.total_shares.checked_sub(shares_to_burn).ok_or(PoolError::ArithmeticOverflow)?;
        state.total_assets = state.total_assets.checked_sub(usdc_to_return).ok_or(PoolError::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);

        let token_client = token::Client::new(&env, &state.usdc_token);
        token_client.transfer(&env.current_contract_address(), &provider, &usdc_to_return);
        events::emit_withdrawn(&env, &provider, shares_to_burn, usdc_to_return);
        Ok(usdc_to_return)
    }

    pub fn allocate_liquidity(env: Env, escrow_caller: Address, merchant_recipient: Address, principal_amount: i128) -> Result<(), PoolError> {
        escrow_caller.require_auth();
        if principal_amount <= 0 {
            return Err(PoolError::InvalidAmount);
        }
        let mut state = Self::load_state(&env)?;
        if state.paused {
            return Err(PoolError::PoolPaused);
        }
        if escrow_caller != state.factoring_escrow {
            return Err(PoolError::Unauthorized);
        }
        let idle_liquidity = state.total_assets.checked_sub(state.deployed_capital).ok_or(PoolError::ArithmeticOverflow)?;
        if principal_amount > idle_liquidity {
            return Err(PoolError::InsufficientPoolLiquidity);
        }
        state.deployed_capital = state.deployed_capital.checked_add(principal_amount).ok_or(PoolError::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);

        let token_client = token::Client::new(&env, &state.usdc_token);
        token_client.transfer(&env.current_contract_address(), &merchant_recipient, &principal_amount);
        events::emit_liquidity_allocated(&env, &escrow_caller, &merchant_recipient, principal_amount);
        Ok(())
    }

    pub fn repay_liquidity(env: Env, escrow_caller: Address, payer: Address, principal_repaid: i128, gross_yield: i128) -> Result<(i128, i128), PoolError> {
        escrow_caller.require_auth();
        payer.require_auth();
        if principal_repaid <= 0 || gross_yield < 0 {
            return Err(PoolError::InvalidAmount);
        }
        let mut state = Self::load_state(&env)?;
        if escrow_caller != state.factoring_escrow {
            return Err(PoolError::Unauthorized);
        }
        if principal_repaid > state.deployed_capital {
            return Err(PoolError::RepaymentExceedsDeployed);
        }
        let protocol_cut = gross_yield.checked_mul(state.reserve_factor_bps as i128).ok_or(PoolError::ArithmeticOverflow)?.checked_div(BPS_DENOMINATOR).ok_or(PoolError::ArithmeticOverflow)?;
        let lp_yield = gross_yield.checked_sub(protocol_cut).ok_or(PoolError::ArithmeticOverflow)?;
        let pool_inflow = principal_repaid.checked_add(lp_yield).ok_or(PoolError::ArithmeticOverflow)?;

        state.deployed_capital = state.deployed_capital.checked_sub(principal_repaid).ok_or(PoolError::ArithmeticOverflow)?;
        state.total_assets = state.total_assets.checked_add(lp_yield).ok_or(PoolError::ArithmeticOverflow)?;
        state.cumulative_yield = state.cumulative_yield.checked_add(lp_yield).ok_or(PoolError::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);

        let token_client = token::Client::new(&env, &state.usdc_token);
        token_client.transfer(&payer, &env.current_contract_address(), &pool_inflow);
        if protocol_cut > 0 {
            token_client.transfer(&payer, &state.treasury, &protocol_cut);
        }
        events::emit_liquidity_repaid(&env, &escrow_caller, principal_repaid, lp_yield, protocol_cut);
        Ok((lp_yield, protocol_cut))
    }

    pub fn write_off_default(env: Env, escrow_caller: Address, defaulted_principal: i128) -> Result<(), PoolError> {
        escrow_caller.require_auth();
        let mut state = Self::load_state(&env)?;
        if escrow_caller != state.factoring_escrow {
            return Err(PoolError::Unauthorized);
        }
        if defaulted_principal <= 0 || defaulted_principal > state.deployed_capital {
            return Err(PoolError::InvalidAmount);
        }
        state.deployed_capital = state.deployed_capital.checked_sub(defaulted_principal).ok_or(PoolError::ArithmeticOverflow)?;
        state.total_assets = state.total_assets.checked_sub(defaulted_principal).unwrap_or(0);
        env.storage().instance().set(&DataKey::State, &state);
        Self::extend_instance_ttl(&env);
        Ok(())
    }

    pub fn get_pool_state(env: Env) -> Result<PoolState, PoolError> {
        Self::load_state(&env)
    }

    pub fn get_position(env: Env, provider: Address) -> LpPosition {
        env.storage().persistent().get(&DataKey::Position(provider.clone())).unwrap_or(LpPosition {
            provider,
            shares: 0,
            deposited_usdc: 0,
            last_deposit_timestamp: 0,
        })
    }

    pub fn get_share_price(env: Env) -> Result<i128, PoolError> {
        let state = Self::load_state(&env)?;
        if state.total_shares == 0 || state.total_assets == 0 {
            return Ok(SHARE_PRICE_SCALE);
        }
        state.total_assets.checked_mul(SHARE_PRICE_SCALE).ok_or(PoolError::ArithmeticOverflow)?.checked_div(state.total_shares).ok_or(PoolError::ArithmeticOverflow)
    }

    pub fn preview_deposit(env: Env, assets: i128) -> Result<i128, PoolError> {
        let state = Self::load_state(&env)?;
        Self::compute_shares_for_deposit(assets, state.total_assets, state.total_shares)
    }

    pub fn preview_withdraw(env: Env, shares: i128) -> Result<i128, PoolError> {
        let state = Self::load_state(&env)?;
        Self::compute_assets_for_shares(shares, state.total_assets, state.total_shares)
    }

    fn compute_shares_for_deposit(assets: i128, total_assets: i128, total_shares: i128) -> Result<i128, PoolError> {
        if assets <= 0 {
            return Err(PoolError::InvalidAmount);
        }
        if total_shares == 0 || total_assets == 0 {
            return Ok(assets);
        }
        assets.checked_mul(total_shares).ok_or(PoolError::ArithmeticOverflow)?.checked_div(total_assets).ok_or(PoolError::ArithmeticOverflow)
    }

    fn compute_assets_for_shares(shares: i128, total_assets: i128, total_shares: i128) -> Result<i128, PoolError> {
        if shares <= 0 || total_shares <= 0 {
            return Err(PoolError::InvalidAmount);
        }
        shares.checked_mul(total_assets).ok_or(PoolError::ArithmeticOverflow)?.checked_div(total_shares).ok_or(PoolError::ArithmeticOverflow)
    }

    fn load_state(env: &Env) -> Result<PoolState, PoolError> {
        env.storage().instance().get(&DataKey::State).ok_or(PoolError::NotInitialized)
    }

    fn extend_instance_ttl(env: &Env) {
        env.storage().instance().extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }

    fn extend_persistent_ttl(env: &Env, key: &DataKey) {
        env.storage().persistent().extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
    }
}
