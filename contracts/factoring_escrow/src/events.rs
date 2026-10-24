use crate::types::FactoringQuote;
use soroban_sdk::{symbol_short, Address, Env};

pub fn emit_escrow_initialized(env: &Env, admin: &Address, registry: &Address, pool: &Address) {
    env.events().publish(
        (symbol_short!("e_init"), admin.clone()),
        (registry.clone(), pool.clone()),
    );
}

pub fn emit_invoice_factored(
    env: &Env,
    invoice_id: u64,
    merchant: &Address,
    quote: &FactoringQuote,
) {
    env.events().publish(
        (symbol_short!("factored"), invoice_id, merchant.clone()),
        (quote.face_value, quote.net_payout, quote.total_discount),
    );
}

pub fn emit_invoice_settled(
    env: &Env,
    invoice_id: u64,
    payer: &Address,
    face_value: i128,
    lp_yield: i128,
    protocol_cut: i128,
) {
    env.events().publish(
        (symbol_short!("settled"), invoice_id, payer.clone()),
        (face_value, lp_yield, protocol_cut),
    );
}

pub fn emit_invoice_defaulted(env: &Env, invoice_id: u64, defaulted_principal: i128) {
    env.events()
        .publish((symbol_short!("default"), invoice_id), defaulted_principal);
}

pub fn emit_paused_changed(env: &Env, admin: &Address, paused: bool) {
    env.events()
        .publish((symbol_short!("paused"), admin.clone()), paused);
}

pub fn emit_rate_config_updated(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("rate_upd"), admin.clone()), ());
}

pub fn emit_admin_transferred(env: &Env, old_admin: &Address, new_admin: &Address) {
    env.events().publish(
        (symbol_short!("adm_xfer"), old_admin.clone()),
        new_admin.clone(),
    );
}
