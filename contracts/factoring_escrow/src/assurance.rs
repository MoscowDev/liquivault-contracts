//! Deterministic generated coverage. Run `cargo test -p factoring_escrow assurance --locked`.
use super::*;
use soroban_sdk::{testutils::Address as _, BytesN, IntoVal, String, TryFromVal, Val};

#[test]
fn generated_quote_conservation_rounding_and_monotonicity() {
    // Fixed LCG seed makes every generated input reproducible without dependencies.
    let mut seed = 0x4c49515549564155u64;
    for _ in 0..4096 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let face = (seed % 1_000_000_000_000 + 1) as i128;
        let duration = seed % 31_536_000 + 1;
        let rate = (seed % 5000 + 1) as u32;
        let cfg = RateConfig {
            tier_a_annual_bps: rate,
            tier_b_annual_bps: rate,
            tier_c_annual_bps: rate,
            origination_fee_bps: (seed % 1001) as u32,
            grace_period_seconds: 604800,
        };
        assert!(FactoringEscrowContract::validate_rate_config(&cfg).is_ok());
        for tier in [RiskTier::TierA, RiskTier::TierB, RiskTier::TierC] {
            let quote =
                FactoringEscrowContract::compute_quote(7, face, tier, 100, 100 + duration, &cfg)
                    .unwrap();
            assert_eq!(quote.net_payout + quote.total_discount, face);
            assert_eq!(
                quote.time_discount_amount,
                face * rate as i128 * duration as i128 / 315_360_000_000
            );
            assert_eq!(
                quote.origination_fee_amount,
                face * cfg.origination_fee_bps as i128 / 10000
            );
            assert_eq!(
                quote.total_discount,
                quote.time_discount_amount + quote.origination_fee_amount
            );
            assert!(quote.net_payout > 0);
            let later =
                FactoringEscrowContract::compute_quote(7, face, tier, 100, 101 + duration, &cfg)
                    .unwrap();
            assert!(later.total_discount >= quote.total_discount);
            if rate < 5000 {
                let higher = RateConfig {
                    tier_a_annual_bps: rate + 1,
                    tier_b_annual_bps: rate + 1,
                    tier_c_annual_bps: rate + 1,
                    ..cfg.clone()
                };
                let next = FactoringEscrowContract::compute_quote(
                    7,
                    face,
                    tier,
                    100,
                    100 + duration,
                    &higher,
                )
                .unwrap();
                assert!(next.total_discount >= quote.total_discount);
            }
        }
    }
}

#[test]
fn quote_extremes_return_errors() {
    let cfg = RateConfig {
        tier_a_annual_bps: 600,
        tier_b_annual_bps: 1000,
        tier_c_annual_bps: 5000,
        origination_fee_bps: 1000,
        grace_period_seconds: 0,
    };
    assert_eq!(
        FactoringEscrowContract::compute_quote(1, 100, RiskTier::TierA, 10, 10, &cfg),
        Err(EscrowError::InvoiceAlreadyExpired)
    );
    assert_eq!(
        FactoringEscrowContract::compute_quote(1, i128::MAX, RiskTier::TierC, 0, u64::MAX, &cfg),
        Err(EscrowError::ArithmeticOverflow)
    );
    assert_eq!(
        FactoringEscrowContract::compute_quote(1, 10000, RiskTier::TierC, 0, 31_536_000 * 2, &cfg),
        Err(EscrowError::DiscountExceedsFaceValue)
    );
    for invalid in [
        RateConfig {
            tier_a_annual_bps: 0,
            ..cfg.clone()
        },
        RateConfig {
            tier_c_annual_bps: 5001,
            ..cfg.clone()
        },
        RateConfig {
            origination_fee_bps: 1001,
            ..cfg.clone()
        },
        RateConfig {
            tier_b_annual_bps: 599,
            ..cfg.clone()
        },
    ] {
        assert_eq!(
            FactoringEscrowContract::validate_rate_config(&invalid),
            Err(EscrowError::InvalidRateConfiguration)
        );
    }
}

#[test]
fn registry_and_escrow_wire_records_round_trip_all_variants() {
    use invoice_registry::types as registry;
    let env = Env::default();
    for (risk, local_risk) in [
        (registry::RiskTier::TierA, RiskTier::TierA),
        (registry::RiskTier::TierB, RiskTier::TierB),
        (registry::RiskTier::TierC, RiskTier::TierC),
    ] {
        for (status, local_status) in [
            (registry::InvoiceStatus::Issued, InvoiceStatus::Issued),
            (registry::InvoiceStatus::Verified, InvoiceStatus::Verified),
            (registry::InvoiceStatus::Factored, InvoiceStatus::Factored),
            (registry::InvoiceStatus::Settled, InvoiceStatus::Settled),
            (registry::InvoiceStatus::Defaulted, InvoiceStatus::Defaulted),
        ] {
            let record = registry::InvoiceRecord {
                invoice_id: u64::MAX,
                merchant: Address::generate(&env),
                owner: Address::generate(&env),
                debtor: Address::generate(&env),
                currency: Address::generate(&env),
                face_value: i128::MAX,
                issued_at: 123,
                maturity_timestamp: u64::MAX,
                metadata_hash: BytesN::from_array(&env, &[42; 32]),
                invoice_number: String::from_str(&env, "WIRE-COMPATIBILITY"),
                risk_tier: risk,
                status,
            };
            let wire: Val = record.clone().into_val(&env);
            let mut malformed =
                soroban_sdk::Map::<soroban_sdk::Symbol, Val>::try_from_val(&env, &wire).unwrap();
            malformed.set(
                soroban_sdk::Symbol::new(&env, "invoice_id"),
                String::from_str(&env, "wrong-wire-type").into_val(&env),
            );
            let malformed: Val = malformed.into_val(&env);
            assert!(interfaces::InvoiceRecord::try_from_val(&env, &malformed).is_err());
            let local = interfaces::InvoiceRecord::try_from_val(&env, &wire).unwrap();
            assert_eq!(local.risk_tier, local_risk);
            assert_eq!(local.status, local_status);
            let back: Val = local.into_val(&env);
            assert_eq!(
                registry::InvoiceRecord::try_from_val(&env, &back).unwrap(),
                record
            );
        }
    }
    let unknown: Val = 99u32.into_val(&env);
    assert!(RiskTier::try_from_val(&env, &unknown).is_err());
    assert!(InvoiceStatus::try_from_val(&env, &unknown).is_err());
}
