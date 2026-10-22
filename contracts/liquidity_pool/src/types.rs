use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub admin: Address,
    pub usdc_token: Address,
    pub factoring_escrow: Address,
    pub treasury: Address,
    pub total_assets: i128,
    pub total_shares: i128,
    pub deployed_capital: i128,
    pub cumulative_yield: i128,
    pub reserve_factor_bps: u32,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpPosition {
    pub provider: Address,
    pub shares: i128,
    pub deposited_usdc: i128,
    pub last_deposit_timestamp: u64,
}

/// First-loss capital tranche. Absorbs defaults before the senior tranche (`PoolState`)
/// and receives `junior_yield_bps` of LP yield in exchange.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JuniorState {
    pub total_assets: i128,
    pub total_shares: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    State,
    Position(Address),
    Junior,
    JuniorPosition(Address),
    JuniorYieldBps,
}
