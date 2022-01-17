use near_sdk::{ext_contract, AccountId, Balance, Gas};
use near_sdk::json_types::U128;

pub const NO_DEPOSIT: Balance = 0;
pub const ONE_NEAR: Balance = 10u128.pow(24);

/// hotfix_insuffient_gas_for_mft_resolve_transfer.
pub const GAS_FOR_RESOLVE_TRANSFER: Gas = 20_000_000_000_000;
pub const GAS_FOR_FT_TRANSFER_CALL: Gas = 25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER;

/// Amount of gas for fungible token transfers, increased to 20T to support AS token contracts.
pub const GAS_FOR_FT_TRANSFER: Gas = 20_000_000_000_000;

/// Fee divisor, allowing to provide fee in bps.
pub const FEE_DIVISOR: u32 = 1_000;

/// Ratio divisor, allowing to provide fee in bps.
pub const RATIO_DIVISOR: u128 = 1_000_000;

/// Price precision, allowing to provide fee in bps.
pub const PRICE_PRECISION: u32 = 100_000;

#[ext_contract(ext_self)]
pub trait CrfExchange {
    fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128,
    );
}
