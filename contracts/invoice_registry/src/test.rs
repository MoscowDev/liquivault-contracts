#![cfg(test)]

use crate::types::{InvoiceStatus, RiskTier};
use crate::{InvoiceRegistryContract, InvoiceRegistryContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, BytesN, Env, String};

fn setup_env() -> (
    Env,
    InvoiceRegistryContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    let contract_id = env.register_contract(None, InvoiceRegistryContract);
    let client = InvoiceRegistryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let verifier = Address::generate(&env);
    let escrow = Address::generate(&env);
    let merchant = Address::generate(&env);
    let debtor = Address::generate(&env);
    let currency = Address::generate(&env);

    client.initialize(&admin, &verifier, &escrow);

    (
        env, client, admin, verifier, escrow, merchant, debtor, currency,
    )
}

#[test]
fn test_mint_and_verify_invoice_lifecycle() {
    let (env, client, _admin, verifier, escrow, merchant, debtor, currency) = setup_env();

    let metadata_hash = BytesN::from_array(&env, &[42u8; 32]);
    let inv_num = String::from_str(&env, "INV-2025-0001");
    let face_value = 50_000_0000000i128; // 50,000 USDC (7 decimals)
    let maturity = 1_700_000_000 + 86_400 * 60; // 60 days

    let invoice_id = client.mint_invoice(
        &merchant,
        &debtor,
        &currency,
        &face_value,
        &maturity,
        &metadata_hash,
        &inv_num,
        &RiskTier::TierB,
    );
    assert_eq!(invoice_id, 1);

    let record = client.get_invoice(&invoice_id);
    assert_eq!(record.status, InvoiceStatus::Issued);
    assert_eq!(record.owner, merchant);
    assert_eq!(record.face_value, face_value);

    // Verify ownership & metadata hash
    assert!(client.verify_ownership(&invoice_id, &merchant));
    assert!(!client.verify_ownership(&invoice_id, &debtor));
    assert!(client.verify_metadata_hash(&invoice_id, &metadata_hash));

    // Verify invoice via oracle verifier and upgrade to TierA
    client.verify_invoice(&verifier, &invoice_id, &RiskTier::TierA);
    let verified_record = client.get_invoice(&invoice_id);
    assert_eq!(verified_record.status, InvoiceStatus::Verified);
    assert_eq!(verified_record.risk_tier, RiskTier::TierA);

    // Mark factored by authorized escrow
    client.mark_factored(&escrow, &invoice_id);
    let factored_record = client.get_invoice(&invoice_id);
    assert_eq!(factored_record.status, InvoiceStatus::Factored);
    assert_eq!(factored_record.owner, escrow);

    // Mark settled by authorized escrow
    client.mark_settled(&escrow, &invoice_id);
    let settled_record = client.get_invoice(&invoice_id);
    assert_eq!(settled_record.status, InvoiceStatus::Settled);
}

#[test]
fn test_reject_invalid_maturity_or_unauthorized_escrow() {
    let (env, client, _admin, verifier, _escrow, merchant, debtor, currency) = setup_env();
    let metadata_hash = BytesN::from_array(&env, &[7u8; 32]);
    let inv_num = String::from_str(&env, "INV-2025-0002");

    // Past maturity must fail
    let res = client.try_mint_invoice(
        &merchant,
        &debtor,
        &currency,
        &10_000_0000000i128,
        &1_699_000_000,
        &metadata_hash,
        &inv_num,
        &RiskTier::TierB,
    );
    assert!(res.is_err());

    // Valid mint
    let invoice_id = client.mint_invoice(
        &merchant,
        &debtor,
        &currency,
        &10_000_0000000i128,
        &(1_700_000_000 + 86_400 * 30),
        &metadata_hash,
        &inv_num,
        &RiskTier::TierB,
    );
    client.verify_invoice(&verifier, &invoice_id, &RiskTier::TierB);

    // Unauthorized caller trying to mark factored must fail
    let attacker = Address::generate(&env);
    let unauthorized_res = client.try_mark_factored(&attacker, &invoice_id);
    assert!(unauthorized_res.is_err());
}
