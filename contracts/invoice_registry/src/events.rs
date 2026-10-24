use crate::types::{InvoiceStatus, RiskTier};
use soroban_sdk::{symbol_short, Address, BytesN, Env};

pub fn emit_initialized(env: &Env, admin: &Address, verifier: &Address, escrow: &Address) {
    env.events().publish(
        (symbol_short!("init"), admin.clone()),
        (verifier.clone(), escrow.clone()),
    );
}

pub fn emit_invoice_minted(
    env: &Env,
    invoice_id: u64,
    merchant: &Address,
    debtor: &Address,
    face_value: i128,
    maturity_timestamp: u64,
    metadata_hash: &BytesN<32>,
) {
    env.events().publish(
        (symbol_short!("minted"), invoice_id, merchant.clone()),
        (
            debtor.clone(),
            face_value,
            maturity_timestamp,
            metadata_hash.clone(),
        ),
    );
}

pub fn emit_invoice_verified(env: &Env, invoice_id: u64, verifier: &Address, risk_tier: RiskTier) {
    env.events().publish(
        (symbol_short!("verified"), invoice_id),
        (verifier.clone(), risk_tier),
    );
}

pub fn emit_status_updated(
    env: &Env,
    invoice_id: u64,
    old_status: InvoiceStatus,
    new_status: InvoiceStatus,
) {
    env.events().publish(
        (symbol_short!("status"), invoice_id),
        (old_status, new_status),
    );
}

pub fn emit_ownership_transferred(env: &Env, invoice_id: u64, from: &Address, to: &Address) {
    env.events().publish(
        (symbol_short!("transfer"), invoice_id),
        (from.clone(), to.clone()),
    );
}
