#![cfg(test)]

use crate::types::{EscrowStatus, RateConfig};
use crate::{FactoringEscrowContract, FactoringEscrowContractClient};
use invoice_registry::types::{InvoiceStatus, RiskTier};
use invoice_registry::{InvoiceRegistryContract, InvoiceRegistryContractClient};
use liquidity_pool::{LiquidityPoolContract, LiquidityPoolContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, BytesN, Env, String};

#[test]
fn test_end_to_end_factoring_and_settlement() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    let admin = Address::generate(&env);
    let verifier = Address::generate(&env);
    let treasury = Address::generate(&env);
    let lp_provider = Address::generate(&env);
    let merchant = Address::generate(&env);
    let debtor = Address::generate(&env);

    let usdc_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let usdc_addr = usdc_contract.address();
    let usdc_client = TokenClient::new(&env, &usdc_addr);
    let usdc_admin = StellarAssetClient::new(&env, &usdc_addr);

    let registry_id = env.register_contract(None, InvoiceRegistryContract);
    let pool_id = env.register_contract(None, LiquidityPoolContract);
    let escrow_id = env.register_contract(None, FactoringEscrowContract);

    let registry_client = InvoiceRegistryContractClient::new(&env, &registry_id);
    let pool_client = LiquidityPoolContractClient::new(&env, &pool_id);
    let escrow_client = FactoringEscrowContractClient::new(&env, &escrow_id);

    registry_client.initialize(&admin, &verifier, &escrow_id);
    pool_client.initialize(&admin, &usdc_addr, &escrow_id, &treasury, &1000u32);

    let rate_config = RateConfig {
        tier_a_annual_bps: 600,
        tier_b_annual_bps: 1000,
        tier_c_annual_bps: 1500,
        origination_fee_bps: 50,
        grace_period_seconds: 86_400 * 7,
    };
    escrow_client.initialize(&admin, &registry_id, &pool_id, &usdc_addr, &rate_config);

    usdc_admin.mint(&lp_provider, &250_000_0000000);
    usdc_admin.mint(&debtor, &100_000_0000000);
    pool_client.deposit(&lp_provider, &250_000_0000000);

    let face_value = 100_000_0000000i128;
    let maturity = 1_700_000_000 + 86_400 * 73; // 73 days = 0.20 year
    let metadata_hash = BytesN::from_array(&env, &[99u8; 32]);
    let inv_num = String::from_str(&env, "INV-2025-9001");

    let invoice_id = registry_client.mint_invoice(
        &merchant,
        &debtor,
        &usdc_addr,
        &face_value,
        &maturity,
        &metadata_hash,
        &inv_num,
        &RiskTier::TierB,
    );

    registry_client.verify_invoice(&verifier, &invoice_id, &RiskTier::TierB);

    let quote = escrow_client.factor_invoice(&merchant, &invoice_id);
    assert_eq!(quote.time_discount_amount, 2_000_0000000);
    assert_eq!(quote.origination_fee_amount, 500_0000000);
    assert_eq!(quote.total_discount, 2_500_0000000);
    assert_eq!(quote.net_payout, 97_500_0000000);
    assert_eq!(usdc_client.balance(&merchant), 97_500_0000000);

    let factored_inv = registry_client.get_invoice(&invoice_id);
    assert_eq!(factored_inv.status, InvoiceStatus::Factored);
    assert_eq!(factored_inv.owner, escrow_id);

    env.ledger().with_mut(|li| {
        li.timestamp = maturity;
    });
    let (lp_yield, protocol_cut) = escrow_client.settle_invoice(&debtor, &invoice_id);
    assert_eq!(lp_yield, 2_250_0000000);
    assert_eq!(protocol_cut, 250_0000000);
    assert_eq!(usdc_client.balance(&treasury), 250_0000000);

    let escrow_rec = escrow_client.get_escrow_record(&invoice_id);
    assert_eq!(escrow_rec.status, EscrowStatus::Settled);

    let redeemed = pool_client.withdraw(&lp_provider, &250_000_0000000);
    assert_eq!(redeemed, 252_250_0000000);
}
