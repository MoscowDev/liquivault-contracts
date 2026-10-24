use soroban_sdk::{symbol_short, Address, Env};

pub fn emit_pool_initialized(env: &Env, admin: &Address, usdc_token: &Address) {
    env.events()
        .publish((symbol_short!("p_init"), admin.clone()), usdc_token.clone());
}

pub fn emit_deposited(env: &Env, provider: &Address, amount: i128, shares_minted: i128) {
    env.events().publish(
        (symbol_short!("deposit"), provider.clone()),
        (amount, shares_minted),
    );
}

pub fn emit_withdrawn(env: &Env, provider: &Address, shares_burned: i128, usdc_returned: i128) {
    env.events().publish(
        (symbol_short!("withdraw"), provider.clone()),
        (shares_burned, usdc_returned),
    );
}

pub fn emit_liquidity_allocated(env: &Env, escrow: &Address, recipient: &Address, principal: i128) {
    env.events().publish(
        (symbol_short!("allocate"), escrow.clone()),
        (recipient.clone(), principal),
    );
}

pub fn emit_liquidity_repaid(
    env: &Env,
    escrow: &Address,
    principal_repaid: i128,
    lp_yield: i128,
    protocol_cut: i128,
) {
    env.events().publish(
        (symbol_short!("repaid"), escrow.clone()),
        (principal_repaid, lp_yield, protocol_cut),
    );
}

pub fn emit_junior_deposited(env: &Env, provider: &Address, amount: i128, shares_minted: i128) {
    env.events().publish(
        (symbol_short!("j_deposit"), provider.clone()),
        (amount, shares_minted),
    );
}

pub fn emit_junior_withdrawn(
    env: &Env,
    provider: &Address,
    shares_burned: i128,
    usdc_returned: i128,
) {
    env.events().publish(
        (symbol_short!("j_withdrw"), provider.clone()),
        (shares_burned, usdc_returned),
    );
}

pub fn emit_loss_absorbed(env: &Env, junior_loss: i128, senior_loss: i128) {
    env.events()
        .publish((symbol_short!("loss"),), (junior_loss, senior_loss));
}
