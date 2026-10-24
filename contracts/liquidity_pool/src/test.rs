#![cfg(test)]

use crate::{LiquidityPoolContract, LiquidityPoolContractClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, Env};

fn create_usdc_token<'a>(
    env: &Env,
    admin: &Address,
) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_addr = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = contract_addr.address();
    (
        addr.clone(),
        TokenClient::new(env, &addr),
        StellarAssetClient::new(env, &addr),
    )
}

#[test]
fn test_deposit_allocate_repay_and_yield_distribution() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let escrow = Address::generate(&env);
    let treasury = Address::generate(&env);
    let lp1 = Address::generate(&env);
    let lp2 = Address::generate(&env);
    let merchant = Address::generate(&env);
    let debtor = Address::generate(&env);

    let (usdc_addr, usdc_client, usdc_admin_client) = create_usdc_token(&env, &admin);
    let pool_id = env.register_contract(None, LiquidityPoolContract);
    let pool_client = LiquidityPoolContractClient::new(&env, &pool_id);

    pool_client.initialize(&admin, &usdc_addr, &escrow, &treasury, &1000u32);

    usdc_admin_client.mint(&lp1, &100_000_0000000);
    usdc_admin_client.mint(&lp2, &100_000_0000000);
    usdc_admin_client.mint(&debtor, &50_000_0000000);

    let shares1 = pool_client.deposit(&lp1, &60_000_0000000);
    let shares2 = pool_client.deposit(&lp2, &40_000_0000000);
    assert_eq!(shares1, 60_000_0000000);
    assert_eq!(shares2, 40_000_0000000);
    assert_eq!(pool_client.get_share_price(), 10_000_000);

    pool_client.allocate_liquidity(&escrow, &merchant, &48_000_0000000);
    assert_eq!(usdc_client.balance(&merchant), 48_000_0000000);

    let (lp_yield, protocol_cut) =
        pool_client.repay_liquidity(&escrow, &debtor, &48_000_0000000, &2_000_0000000);
    assert_eq!(lp_yield, 1_800_0000000);
    assert_eq!(protocol_cut, 200_0000000);
    assert_eq!(usdc_client.balance(&treasury), 200_0000000);
    assert_eq!(pool_client.get_share_price(), 10_180_000);

    let redeemed1 = pool_client.withdraw(&lp1, &shares1);
    assert_eq!(redeemed1, 61_080_0000000);

    let redeemed2 = pool_client.withdraw(&lp2, &shares2);
    assert_eq!(redeemed2, 40_720_0000000);
}

#[test]
fn test_set_paused_blocks_deposit_and_withdraw_and_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let escrow = Address::generate(&env);
    let treasury = Address::generate(&env);
    let lp = Address::generate(&env);
    let stranger = Address::generate(&env);

    let (usdc_addr, _usdc_client, usdc_admin_client) = create_usdc_token(&env, &admin);
    let pool_id = env.register_contract(None, LiquidityPoolContract);
    let pool = LiquidityPoolContractClient::new(&env, &pool_id);
    pool.initialize(&admin, &usdc_addr, &escrow, &treasury, &1000u32);
    usdc_admin_client.mint(&lp, &10_000_0000000);

    let shares = pool.deposit(&lp, &5_000_0000000);

    assert!(pool.try_set_paused(&stranger, &true).is_err());
    pool.set_paused(&admin, &true);
    assert!(pool.try_deposit(&lp, &1_000_0000000).is_err());
    assert!(pool.try_withdraw(&lp, &shares).is_err());

    pool.set_paused(&admin, &false);
    pool.withdraw(&lp, &shares);
}

const U: i128 = 10_000_000; // 1 USDC (7 decimals)

struct Pool<'a> {
    env: Env,
    admin: Address,
    escrow: Address,
    merchant: Address,
    payer: Address,
    pool: LiquidityPoolContractClient<'a>,
    senior_lp: Address,
    junior_lp: Address,
}

/// Pool with 60k senior and 20k junior USDC deposited; reserve factor 10%.
fn tranche_setup<'a>() -> Pool<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let escrow = Address::generate(&env);
    let treasury = Address::generate(&env);
    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let senior_lp = Address::generate(&env);
    let junior_lp = Address::generate(&env);

    let (usdc_addr, _c, usdc_admin) = create_usdc_token(&env, &admin);
    let pool_id = env.register_contract(None, LiquidityPoolContract);
    let pool = LiquidityPoolContractClient::new(&env, &pool_id);
    pool.initialize(&admin, &usdc_addr, &escrow, &treasury, &1000u32);

    usdc_admin.mint(&senior_lp, &(100_000 * U));
    usdc_admin.mint(&junior_lp, &(100_000 * U));
    usdc_admin.mint(&payer, &(100_000 * U));
    pool.deposit(&senior_lp, &(60_000 * U));
    pool.deposit_junior(&junior_lp, &(20_000 * U));

    Pool {
        env,
        admin,
        escrow,
        merchant,
        payer,
        pool,
        senior_lp,
        junior_lp,
    }
}

#[test]
fn test_junior_receives_share_of_yield() {
    let p = tranche_setup();
    p.pool
        .allocate_liquidity(&p.escrow, &p.merchant, &(40_000 * U));
    // 2,000 gross yield -> 200 protocol cut -> 1,800 LP yield -> 30% junior (540), 70% senior (1,260).
    p.pool
        .repay_liquidity(&p.escrow, &p.payer, &(40_000 * U), &(2_000 * U));

    assert_eq!(p.pool.get_junior_state().total_assets, 20_540 * U);
    assert_eq!(p.pool.get_pool_state().total_assets, 61_260 * U);
    assert_eq!(p.pool.get_pool_state().cumulative_yield, 1_800 * U);
}

#[test]
fn test_junior_absorbs_losses_before_senior() {
    let p = tranche_setup();
    p.pool
        .allocate_liquidity(&p.escrow, &p.merchant, &(40_000 * U));

    // Loss smaller than junior capital: senior untouched.
    p.pool.write_off_default(&p.escrow, &(15_000 * U));
    assert_eq!(p.pool.get_junior_state().total_assets, 5_000 * U);
    assert_eq!(p.pool.get_pool_state().total_assets, 60_000 * U);

    // Remaining 10k loss: junior's last 5k, then 5k from senior.
    p.pool.write_off_default(&p.escrow, &(10_000 * U));
    assert_eq!(p.pool.get_junior_state().total_assets, 0);
    assert_eq!(p.pool.get_pool_state().total_assets, 55_000 * U);
    assert_eq!(p.pool.get_pool_state().deployed_capital, 15_000 * U);
}

#[test]
fn test_wiped_out_junior_rejects_new_deposits_and_senior_gets_all_yield() {
    let p = tranche_setup();
    p.pool
        .allocate_liquidity(&p.escrow, &p.merchant, &(40_000 * U));
    p.pool.write_off_default(&p.escrow, &(20_000 * U));
    assert_eq!(p.pool.get_junior_state().total_assets, 0);

    // Stale junior shares must not dilute a fresh depositor.
    assert!(p
        .pool
        .try_deposit_junior(&p.junior_lp, &(1_000 * U))
        .is_err());

    // With no junior capital, senior takes all LP yield.
    p.pool
        .repay_liquidity(&p.escrow, &p.payer, &(20_000 * U), &(1_000 * U));
    assert_eq!(p.pool.get_pool_state().total_assets, 60_000 * U + 900 * U);
    assert_eq!(p.pool.get_junior_state().total_assets, 0);
}

#[test]
fn test_junior_withdrawal_requires_coverage_of_deployed_capital() {
    let p = tranche_setup();
    p.pool
        .allocate_liquidity(&p.escrow, &p.merchant, &(50_000 * U));
    let shares = p.pool.get_junior_position(&p.junior_lp).shares;

    // Withdrawing everything leaves 0 < 10% of 50k deployed.
    assert!(p.pool.try_withdraw_junior(&p.junior_lp, &shares).is_err());
    // Withdrawing half leaves 10k >= 5k required.
    let returned = p.pool.withdraw_junior(&p.junior_lp, &(shares / 2));
    assert_eq!(returned, 10_000 * U);
    // Withdrawing the rest is still blocked while capital is deployed.
    assert!(p
        .pool
        .try_withdraw_junior(&p.junior_lp, &(shares / 2))
        .is_err());

    // After repayment, junior can exit fully.
    p.pool
        .repay_liquidity(&p.escrow, &p.payer, &(50_000 * U), &(0));
    p.pool.withdraw_junior(&p.junior_lp, &(shares / 2));
    assert_eq!(p.pool.get_junior_state().total_shares, 0);
}

#[test]
fn test_set_junior_yield_bps_admin_only_and_capped() {
    let p = tranche_setup();
    let stranger = Address::generate(&p.env);
    assert!(p
        .pool
        .try_set_junior_yield_bps(&stranger, &1_000u32)
        .is_err());
    assert!(p
        .pool
        .try_set_junior_yield_bps(&p.admin, &9_000u32)
        .is_err());
    p.pool.set_junior_yield_bps(&p.admin, &5_000u32);

    p.pool
        .allocate_liquidity(&p.escrow, &p.merchant, &(40_000 * U));
    p.pool
        .repay_liquidity(&p.escrow, &p.payer, &(40_000 * U), &(2_000 * U));
    // 1,800 LP yield, 50% to junior = 900.
    assert_eq!(p.pool.get_junior_state().total_assets, 20_900 * U);
    let _ = &p.senior_lp;
}
