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
