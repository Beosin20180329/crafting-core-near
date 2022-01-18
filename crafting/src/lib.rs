use std::fmt;

use near_sdk::{
    assert_one_yocto, env, log, near_bindgen, AccountId, Balance, BlockHeight, Timestamp,
    PanicOnDefault, Promise, PromiseResult, StorageUsage, BorshStorageKey,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet, Vector};
use near_sdk::json_types::{ValidAccountId};
use near_sdk::serde::{Deserialize, Serialize};

use crate::account::{VAccount, Account};

mod account;
mod accountbook;
mod debtpool;
mod errors;
mod oracle;
mod owner;
mod utils;
mod views;

near_sdk::setup_alloc!();

pub type CollateralId = u64;

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Accounts,
    Whitelist,
    AccountTokens { account_id: AccountId },
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum RunningState {
    Running,
    Paused,
}

impl fmt::Display for RunningState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunningState::Running => write!(f, "Running"),
            RunningState::Paused => write!(f, "Paused"),
        }
    }
}

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Collateral {
    issuer: AccountId,
    token: AccountId,
    token_amount: Balance,
    raft: AccountId,
    raft_amount: Balance,
    join_debtpool: bool,
    block_index: BlockHeight,
    create_time: Timestamp,
    state: u8,
}

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Asset {
    name: String,
    symbol: String,
    standard: String,
    decimals: u32,
    address: AccountId,
    feed_address: AccountId,
    collateral_ratio: u128,
    state: u8,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    /// Account of the owner.
    owner_id: AccountId,
    /// Running state
    state: RunningState,
    /// Leverage ratio (managed by governance).
    leverage_ratio: (u8, u8),
    /// Interest fee (managed by governance).
    interest_fee: u32,
    /// Exchange fee (managed by governance).
    exchange_fee: u32,
    /// Accounts registered, keeping track all the amounts deposited, storage and more.
    accounts: LookupMap<AccountId, VAccount>,
    /// Set of whitelisted tokens by "owner".
    whitelisted_tokens: UnorderedSet<AccountId>,
    token_list: UnorderedMap<AccountId, Asset>,
    /// Set of whitelisted rafts by "owner".
    whitelisted_rafts: UnorderedSet<AccountId>,
    raft_list: UnorderedMap<AccountId, Asset>,
    /// Collateral
    collaterals: Vector<Collateral>,
    user_collaterals: LookupMap<AccountId, Vector<CollateralId>>,
    /// Debt pool
    debt_pool: debtpool::DebtPool,
    /// Account book
    account_book: accountbook::AccountBook,
    /// Oracle
    price_oracle: oracle::PriceInfo,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: ValidAccountId) -> Self {
        Self {
            owner_id: owner_id.as_ref().clone(),
            state: RunningState::Running,
            leverage_ratio: (1, 10),
            interest_fee: 0,
            exchange_fee: 3,
            accounts: LookupMap::new(StorageKey::Accounts),
            whitelisted_tokens: UnorderedSet::new(b"r".to_vec()),
            token_list: UnorderedMap::new(b"r".to_vec()),
            whitelisted_rafts: UnorderedSet::new(b"r".to_vec()),
            raft_list: UnorderedMap::new(b"r".to_vec()),
            collaterals: Vector::new(b"r".to_vec()),
            user_collaterals: LookupMap::new(b"r".to_vec()),
            debt_pool: debtpool::DebtPool::new(),
            account_book: accountbook::AccountBook::new(),
            price_oracle: oracle::PriceInfo::new(),
        }
    }

    #[payable]
    pub fn mint(&mut self, token: ValidAccountId, raft: ValidAccountId, raft_amount: Balance, join_debtpool: bool) {
        assert_one_yocto();
        self.assert_contract_running();

        assert!(self.is_in_whitelisted_tokens(token.as_ref()));
        assert!(self.is_in_whitelisted_rafts(raft.as_ref()));

        let token_amount = env::attached_deposit();
        assert!(token_amount > 0, "{}", errors::NoAttachedDeposit);
        assert!(raft_amount > 0, "{}", errors::SyntheticAmountError);

        let caller = env::predecessor_account_id();
        if join_debtpool {
            let token_decimals = self.query_token(token.as_ref()).unwrap().decimals;
            let raft_decimals = self.query_raft(raft.as_ref()).unwrap().decimals;

            let leverage_ratio = (self.price_oracle.get_price(raft.as_ref()) * raft_amount * 10u128.pow(token_decimals))
                / (self.price_oracle.get_price(token.as_ref()) * token_amount * 10u128.pow(raft_decimals));

            let (min, max) = self.leverage_ratio;
            assert!(leverage_ratio >= min.into());
            assert!(leverage_ratio <= max.into());

            self.debt_pool.join(&self.price_oracle, &caller, raft.as_ref(), raft_amount);
        } else {
            let token_asset = self.query_token(token.as_ref()).unwrap();
            let raft_asset = self.query_token(raft.as_ref()).unwrap();

            let token_decimals = token_asset.decimals;
            let raft_decimals = raft_asset.decimals;

            let collateral_ratio = (self.price_oracle.get_price(token.as_ref()) * token_amount * 10u128.pow(raft_decimals) * 100)
                / (self.price_oracle.get_price(raft.as_ref()) * raft_amount * 10u128.pow(token_decimals));

            assert!(collateral_ratio >= token_asset.collateral_ratio);

            self.account_book.mint(&caller, raft.as_ref(), raft_amount);
        }

        let collateral = Collateral {
            issuer: caller,
            token: token.as_ref().clone(),
            token_amount,
            raft: raft.as_ref().clone(),
            raft_amount,
            join_debtpool,
            block_index: env::block_index(),
            create_time: env::block_timestamp(),
            state: 0,
        };

        self.collaterals.push(&collateral);
    }

    pub fn swap_in_debtpool(&mut self, old_raft: ValidAccountId, new_raft: ValidAccountId, swap_amount: Balance) {
        self.assert_contract_running();

        assert!(self.is_in_whitelisted_rafts(old_raft.as_ref()));
        assert!(self.is_in_whitelisted_rafts(new_raft.as_ref()));
        assert!(swap_amount > 0);

        let caller = env::predecessor_account_id();

        let old_raft_amount = self.debt_pool.query_raft_amount(old_raft.as_ref());
        assert!(old_raft_amount >= swap_amount);
        let old_user_raft_amount = self.debt_pool.query_user_raft_amount(&caller, old_raft.as_ref());
        assert!(old_user_raft_amount >= swap_amount);

        // charge transaction fee
        let exchange_fee_amount = swap_amount * self.exchange_fee as u128 / utils::FEE_DIVISOR as u128;
        let owner_raft_amount = self.debt_pool.query_user_raft_amount(&self.owner_id, old_raft.as_ref());
        self.debt_pool.insert_user_raft_amount(&self.owner_id, old_raft.as_ref(), owner_raft_amount + exchange_fee_amount);

        self.debt_pool.insert_raft_amount(old_raft.as_ref(), old_raft_amount - swap_amount + exchange_fee_amount);
        self.debt_pool.insert_user_raft_amount(&caller, old_raft.as_ref(), old_user_raft_amount - swap_amount);

        let new_swap_amount = self.debt_pool.calc_raft_value(&self.price_oracle, old_raft.as_ref(), swap_amount - exchange_fee_amount)
            / &self.price_oracle.get_price(new_raft.as_ref());
        let new_raft_amount = self.debt_pool.query_raft_amount(new_raft.as_ref());
        self.debt_pool.insert_raft_amount(new_raft.as_ref(), new_raft_amount + new_swap_amount);

        let new_user_raft_amount = self.debt_pool.query_user_raft_amount(&caller, new_raft.as_ref());
        self.debt_pool.insert_user_raft_amount(&caller, new_raft.as_ref(), new_user_raft_amount + new_swap_amount);
    }

    #[payable]
    pub fn redeem_in_debtpool(&mut self) {
        assert_one_yocto();
        self.assert_contract_running();

        let opt_rusd = self.query_rusd();
        assert!(opt_rusd.is_some());
        let rusd_asset = opt_rusd.unwrap();

        // calculate user debt
        let caller = env::predecessor_account_id();
        let user_debt_ratio = self.debt_pool.query_debt_ratio(&caller);
        let raft_total_value = self.debt_pool.calc_raft_total_value(&self.price_oracle);
        let user_debt = raft_total_value * user_debt_ratio / utils::RATIO_DIVISOR;

        if user_debt > 0 {
            let user_rusd_amount_in_debtpool = self.debt_pool.query_user_raft_amount(&caller, &rusd_asset.address);
            if user_debt <= user_rusd_amount_in_debtpool * utils::PRICE_PRECISION as u128 {
                // subtract user raft amount
                self.debt_pool.insert_user_raft_amount(&caller, &rusd_asset.address, user_rusd_amount_in_debtpool - user_debt / utils::PRICE_PRECISION as u128);

                // subtract total raft amount
                let rusd_amount = self.debt_pool.query_raft_amount(&rusd_asset.address);
                self.debt_pool.insert_raft_amount(&rusd_asset.address, rusd_amount - user_debt / utils::PRICE_PRECISION as u128);

                // remove user debt ratio
                self.debt_pool.remove_debt_ratio(&caller);

                // recalculating debt ratio
                self.debt_pool.calc_all_debt_ratio(raft_total_value, raft_total_value - user_debt);
            } else {
                let user_rusd_amount_in_accountbook = self.account_book.query_user_raft_amount(&caller, &rusd_asset.address);
                assert!(user_debt <= (user_rusd_amount_in_debtpool + user_rusd_amount_in_accountbook) * utils::PRICE_PRECISION as u128);

                // remove user raft amount in debt pool
                self.debt_pool.remove_user_raft_amount(&caller, &rusd_asset.address);

                // subtract total raft amount in debt pool
                let rusd_amount_in_debtpool = self.debt_pool.query_raft_amount(&rusd_asset.address);
                self.debt_pool.insert_raft_amount(&rusd_asset.address, rusd_amount_in_debtpool - user_debt / utils::PRICE_PRECISION as u128);

                // remove user debt ratio
                self.debt_pool.remove_debt_ratio(&caller);

                // recalculating debt ratio
                self.debt_pool.calc_all_debt_ratio(raft_total_value, raft_total_value - user_debt);

                let remaining_debt_amount = user_debt / utils::PRICE_PRECISION as u128 - user_rusd_amount_in_debtpool;
                // subtract user raft amount in account book
                self.account_book.insert_user_raft_amount(&caller, &rusd_asset.address, user_rusd_amount_in_accountbook - remaining_debt_amount);

                // subtract total raft amount in account book
                let rusd_amount_in_accountbook = self.account_book.query_raft_amount(&rusd_asset.address);
                self.account_book.insert_raft_amount(&rusd_asset.address, rusd_amount_in_accountbook - remaining_debt_amount);
            }
        }

        // transfer debt pool assets to account book
        for (raft, amount) in self.debt_pool.query_user_raft_amounts(&caller).iter() {
            let dp_amount = self.debt_pool.query_raft_amount(raft);
            self.debt_pool.insert_raft_amount(raft, dp_amount - amount);
            self.debt_pool.remove_user_raft_amount(&caller, raft);

            let ab_amount = self.account_book.query_raft_amount(raft);
            self.account_book.insert_raft_amount(raft, ab_amount + amount);

            let ab_user_amount = self.account_book.query_user_raft_amount(&caller, raft);
            self.account_book.insert_user_raft_amount(&caller, raft, ab_user_amount + amount);
        }

        // return of collateral assets
        let collateral_ids: Option<Vector<CollateralId>> = self.user_collaterals.get(&caller);
        if collateral_ids.is_some() {
            for collateral_id in collateral_ids.unwrap().iter() {
                let opt_collateral = self.query_collateral(collateral_id);
                if opt_collateral.is_none() { continue; }
                let collateral = opt_collateral.unwrap();

                let mut account = self.internal_unwrap_account(&caller);
                account.withdraw(&collateral.token, collateral.token_amount);
                self.internal_save_account(&caller, account);
                self.internal_send_tokens(&caller, &collateral.token, collateral.token_amount);
            }
        }
    }

    #[payable]
    pub fn redeem_in_accountbook(&mut self, collateral_id: CollateralId) {
        assert_one_yocto();
        self.assert_contract_running();

        let opt_collateral = self.query_collateral(collateral_id);
        assert!(opt_collateral.is_some());

        let caller = env::predecessor_account_id();
        let collateral = opt_collateral.unwrap();
        assert_eq!(collateral.issuer, caller);
        assert_eq!(collateral.join_debtpool, false);
        assert_eq!(collateral.state, 0);

        let raft_amount = self.account_book.query_raft_amount(&collateral.raft);
        let user_raft_amount = self.account_book.query_user_raft_amount(&caller, &collateral.raft);
        let interest_fee_amount = collateral.raft_amount * self.interest_fee as u128 / utils::FEE_DIVISOR as u128;
        assert!(raft_amount > collateral.raft_amount + interest_fee_amount);
        assert!(user_raft_amount > collateral.raft_amount + interest_fee_amount);

        // charge interest fee
        let owner_raft_amount = self.account_book.query_user_raft_amount(&self.owner_id, &collateral.raft);
        self.account_book.insert_user_raft_amount(&self.owner_id, &collateral.raft, owner_raft_amount + interest_fee_amount);

        // subtract user raft amount
        self.account_book.insert_user_raft_amount(&caller, &collateral.raft, user_raft_amount - collateral.raft_amount);

        // subtract total raft amount
        self.account_book.insert_raft_amount(&collateral.raft, raft_amount - collateral.raft_amount);

        let mut account = self.internal_unwrap_account(&caller);
        account.withdraw(&collateral.token, collateral.token_amount);
        self.internal_save_account(&caller, account);
        self.internal_send_tokens(&caller, &collateral.token, collateral.token_amount);
    }
}

/// Internal methods implementation.
impl Contract {
    fn assert_contract_running(&self) {
        match self.state {
            RunningState::Running => (),
            _ => env::panic(errors::CONTRACT_PAUSED.as_bytes()),
        };
    }

    fn is_in_whitelisted_tokens(&self, token: &AccountId) -> bool {
        if self.whitelisted_tokens.contains(token) {
            return true;
        }

        false
    }

    fn query_token(&self, token: &AccountId) -> Option<Asset> {
        self.token_list.get(token)
    }

    fn is_in_whitelisted_rafts(&self, raft: &AccountId) -> bool {
        if self.whitelisted_rafts.contains(raft) {
            return true;
        }

        false
    }

    fn query_raft(&self, raft: &AccountId) -> Option<Asset> {
        self.raft_list.get(raft)
    }

    fn query_rusd(&self) -> Option<Asset> {
        for (_, asset) in self.raft_list.iter() {
            if asset.symbol == "rUSD" {
                return Some(asset);
            }
        }

        None
    }

    fn query_collateral(&self, collateral_id: CollateralId) -> Option<Collateral> {
        self.collaterals.get(collateral_id)
    }

    fn assert_query_authority(&self, user: AccountId) {
        if self.owner_id == env::predecessor_account_id() {
            return;
        }

        assert_eq!(user, env::predecessor_account_id(), "{}", errors::NoPermission);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
