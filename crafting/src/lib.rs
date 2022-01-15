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
    AccountTokens {account_id: AccountId},
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
        assert!(self.is_in_whitelisted_tokens(token.as_ref().clone()));
        assert!(self.is_in_whitelisted_rafts(raft.as_ref().clone()));

        let token_amount = env::attached_deposit();
        assert!(token_amount > 0, "{}", errors::NoAttachedDeposit);
        assert!(raft_amount > 0, "{}", errors::SyntheticAmountError);

        let caller = env::predecessor_account_id();
        if join_debtpool {
            let token_decimals = self.query_token(token.as_ref().clone()).unwrap().decimals;
            let raft_decimals = self.query_raft(raft.as_ref().clone()).unwrap().decimals;

            let leverage_ratio = (self.price_oracle.get_price(raft.as_ref().clone()) * raft_amount * 10u128.pow(token_decimals))
                 / (self.price_oracle.get_price(token.as_ref().clone()) * token_amount * 10u128.pow(raft_decimals));

            let (min, max) = self.leverage_ratio;
            assert!(leverage_ratio >= min.into());
            assert!(leverage_ratio <= max.into());

            self.debt_pool.join(&self.price_oracle, caller.clone(), raft.as_ref().clone(), raft_amount);
        } else {
            let token_asset = self.query_token(token.as_ref().clone()).unwrap();
            let raft_asset = self.query_token(raft.as_ref().clone()).unwrap();

            let token_decimals = token_asset.decimals;
            let raft_decimals = raft_asset.decimals;

            let collateral_ratio = (self.price_oracle.get_price(token.as_ref().clone()) * token_amount * 10u128.pow(raft_decimals) * 100)
                / (self.price_oracle.get_price(raft.as_ref().clone()) * raft_amount * 10u128.pow(token_decimals));

            assert!(collateral_ratio >= token_asset.collateral_ratio);

            self.account_book.mint(caller.clone(), raft.as_ref().clone(), raft_amount);
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

    pub fn swap(&mut self, old_raft: ValidAccountId, new_raft: ValidAccountId, swap_amount: Balance) {
        assert!(self.is_in_whitelisted_rafts(old_raft.as_ref().clone()));
        assert!(self.is_in_whitelisted_rafts(new_raft.as_ref().clone()));
        assert!(swap_amount > 0);

        let caller = env::predecessor_account_id();
        self.debt_pool.swap(caller, old_raft.as_ref().clone(),
                            new_raft.as_ref().clone(), swap_amount,
                            &self.price_oracle,self.owner_id.clone(), self.exchange_fee);
    }

    #[payable]
    pub fn redeem_not_in_debtpool(&mut self, collateral_id: CollateralId) -> Promise {
        assert_one_yocto();
        self.assert_contract_running();

        let opt_collateral = self.query_collateral(collateral_id);
        assert!(opt_collateral.is_some());

        let caller = env::predecessor_account_id();
        let collateral = opt_collateral.unwrap();
        assert_eq!(collateral.issuer, caller);
        assert_eq!(collateral.join_debtpool, false);
        assert_eq!(collateral.state, 0);

        let raft_amount = self.account_book.query_raft_amount(collateral.raft.clone());
        let user_raft_amount = self.account_book.query_user_raft_amount(caller.clone(), collateral.raft.clone());
        let interest_fee_amount = collateral.raft_amount * self.interest_fee as u128 / utils::FEE_DIVISOR as u128;
        assert!(raft_amount > collateral.raft_amount + interest_fee_amount);
        assert!(user_raft_amount > collateral.raft_amount + interest_fee_amount);

        // charge interest fee
        let owner_raft_amount = self.account_book.query_user_raft_amount(self.owner_id.clone(), collateral.raft.clone());
        self.account_book.insert_user_raft_amount(self.owner_id.clone(), collateral.raft.clone(), owner_raft_amount + interest_fee_amount);

        // subtract user raft amount
        self.account_book.insert_user_raft_amount(caller.clone(), collateral.raft.clone(), user_raft_amount - collateral.raft_amount);

        // subtract total raft amount
        self.account_book.insert_raft_amount(collateral.raft.clone(), raft_amount - collateral.raft_amount);

        let mut account = self.internal_unwrap_account(&caller);
        account.withdraw(&collateral.token, collateral.token_amount);
        self.internal_save_account(&caller, account);
        self.internal_send_tokens(&caller, &collateral.token, amount)
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

    fn is_in_whitelisted_tokens(&self, token: AccountId) -> bool {
        if self.whitelisted_tokens.contains(&token) {
            return true;
        }

        false
    }

    fn query_token(&self, token: AccountId) -> Option<Asset> {
        self.token_list.get(&token)
    }

    fn is_in_whitelisted_rafts(&self, raft: AccountId) -> bool {
        if self.whitelisted_rafts.contains(&raft) {
            return true;
        }

        false
    }

    fn query_raft(&self, raft: AccountId) -> Option<Asset> {
        self.raft_list.get(&raft)
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