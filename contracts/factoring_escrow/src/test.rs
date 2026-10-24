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

struct Ctx<'a> {
    env: Env,
    admin: Address,
    verifier: Address,
    merchant: Address,
    debtor: Address,
    usdc: Address,
    registry: InvoiceRegistryContractClient<'a>,
    escrow: FactoringEscrowContractClient<'a>,
}

fn rates() -> RateConfig {
    RateConfig {
        tier_a_annual_bps: 600,
        tier_b_annual_bps: 1000,
        tier_c_annual_bps: 1500,
        origination_fee_bps: 50,
        grace_period_seconds: 86_400 * 7,
    }
}

fn setup<'a>() -> Ctx<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 1_700_000_000);

    let admin = Address::generate(&env);
    let verifier = Address::generate(&env);
    let treasury = Address::generate(&env);
    let lp_provider = Address::generate(&env);
    let merchant = Address::generate(&env);
    let debtor = Address::generate(&env);

    let usdc = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let usdc_admin = StellarAssetClient::new(&env, &usdc);

    let registry_id = env.register_contract(None, InvoiceRegistryContract);
    let pool_id = env.register_contract(None, LiquidityPoolContract);
    let escrow_id = env.register_contract(None, FactoringEscrowContract);

    let registry = InvoiceRegistryContractClient::new(&env, &registry_id);
    let pool = LiquidityPoolContractClient::new(&env, &pool_id);
    let escrow = FactoringEscrowContractClient::new(&env, &escrow_id);

    registry.initialize(&admin, &verifier, &escrow_id);
    pool.initialize(&admin, &usdc, &escrow_id, &treasury, &1000u32);
    escrow.initialize(&admin, &registry_id, &pool_id, &usdc, &rates());

    usdc_admin.mint(&lp_provider, &250_000_0000000);
    usdc_admin.mint(&debtor, &100_000_0000000);
    pool.deposit(&lp_provider, &250_000_0000000);

    Ctx {
        env,
        admin,
        verifier,
        merchant,
        debtor,
        usdc,
        registry,
        escrow,
    }
}

fn verified_invoice(c: &Ctx, maturity: u64) -> u64 {
    let id = c.registry.mint_invoice(
        &c.merchant,
        &c.debtor,
        &c.usdc,
        &100_000_0000000i128,
        &maturity,
        &BytesN::from_array(&c.env, &[7u8; 32]),
        &String::from_str(&c.env, "INV-T"),
        &RiskTier::TierB,
    );
    c.registry
        .verify_invoice(&c.verifier, &id, &RiskTier::TierB);
    id
}

#[test]
fn test_set_paused_blocks_factoring_and_settlement() {
    let c = setup();
    let id = verified_invoice(&c, 1_700_000_000 + 86_400 * 30);
    c.escrow.set_paused(&c.admin, &true);
    assert!(c.escrow.try_factor_invoice(&c.merchant, &id).is_err());

    c.escrow.set_paused(&c.admin, &false);
    c.escrow.factor_invoice(&c.merchant, &id);

    c.escrow.set_paused(&c.admin, &true);
    assert!(c.escrow.try_settle_invoice(&c.debtor, &id).is_err());
}

#[test]
fn test_set_paused_rejects_non_admin() {
    let c = setup();
    let stranger = Address::generate(&c.env);
    assert!(c.escrow.try_set_paused(&stranger, &true).is_err());
}

#[test]
fn test_update_rate_config_changes_quote_and_validates() {
    let c = setup();
    let id = verified_invoice(&c, 1_700_000_000 + 86_400 * 73);
    let before = c.escrow.preview_factoring_quote(&id);

    let mut new_rates = rates();
    new_rates.tier_b_annual_bps = 1200;
    c.escrow.update_rate_config(&c.admin, &new_rates);
    let after = c.escrow.preview_factoring_quote(&id);
    assert!(after.total_discount > before.total_discount);

    let mut bad = rates();
    bad.tier_c_annual_bps = 6_000;
    assert!(c.escrow.try_update_rate_config(&c.admin, &bad).is_err());

    let stranger = Address::generate(&c.env);
    assert!(c
        .escrow
        .try_update_rate_config(&stranger, &new_rates)
        .is_err());
}

#[test]
fn test_transfer_admin() {
    let c = setup();
    let new_admin = Address::generate(&c.env);
    c.escrow.transfer_admin(&c.admin, &new_admin);
    assert!(c.escrow.try_set_paused(&c.admin, &true).is_err());
    c.escrow.set_paused(&new_admin, &true);
}

#[test]
fn test_factor_rejects_wrong_owner_and_double_factoring() {
    let c = setup();
    let id = verified_invoice(&c, 1_700_000_000 + 86_400 * 30);
    let other = Address::generate(&c.env);
    assert!(c.escrow.try_factor_invoice(&other, &id).is_err());
    c.escrow.factor_invoice(&c.merchant, &id);
    assert!(c.escrow.try_factor_invoice(&c.merchant, &id).is_err());
}

#[test]
fn test_factor_rejects_expired_invoice() {
    let c = setup();
    let id = verified_invoice(&c, 1_700_000_000 + 100);
    c.env
        .ledger()
        .with_mut(|li| li.timestamp = 1_700_000_000 + 200);
    assert!(c.escrow.try_factor_invoice(&c.merchant, &id).is_err());
}

#[test]
fn test_default_requires_grace_period_and_admin() {
    let c = setup();
    let maturity = 1_700_000_000 + 86_400 * 30;
    let id = verified_invoice(&c, maturity);
    c.escrow.factor_invoice(&c.merchant, &id);

    let deadline = maturity + 86_400 * 7;
    c.env.ledger().with_mut(|li| li.timestamp = deadline);
    assert!(c.escrow.try_mark_defaulted(&c.admin, &id).is_err());

    c.env.ledger().with_mut(|li| li.timestamp = deadline + 1);
    let stranger = Address::generate(&c.env);
    assert!(c.escrow.try_mark_defaulted(&stranger, &id).is_err());

    c.escrow.mark_defaulted(&c.admin, &id);
    assert_eq!(
        c.escrow.get_escrow_record(&id).status,
        EscrowStatus::Defaulted
    );
    assert_eq!(c.registry.get_invoice(&id).status, InvoiceStatus::Defaulted);
    assert_eq!(c.escrow.get_protocol_metrics().1, 0);
    assert!(c.escrow.try_settle_invoice(&c.debtor, &id).is_err());
}

#[test]
fn test_calculate_discount_matches_spec() {
    let c = setup();
    let q = c.escrow.calculate_discount(
        &1,
        &100_000_0000000i128,
        &crate::interfaces::RiskTier::TierA,
        &(1_700_000_000 + 86_400 * 73),
    );
    assert_eq!(q.time_discount_amount, 1_200_0000000);
    assert_eq!(q.origination_fee_amount, 500_0000000);
    assert_eq!(q.net_payout, 98_300_0000000);
}
