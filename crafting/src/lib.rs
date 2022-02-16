use std::fmt;

use near_sdk::{
    assert_one_yocto, env, near_bindgen, ext_contract, AccountId, Balance, BlockHeight, Timestamp,
    PanicOnDefault, Promise, PromiseOrValue, BorshStorageKey,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet, Vector};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_contract_standards::fungible_token::core_impl::ext_fungible_token;

use crate::account::VAccount;

mod account;
mod accountbook;
mod debtpool;
mod errors;
mod oracle;
mod owner;
mod utils;
mod views;

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

#[ext_contract(ext_enhanced_fungible_token)]
pub trait EnhancedFungibleTokenContract {
    fn mint(&mut self, account_id: AccountId, amount: U128);

    fn burn(&mut self, account_id: AccountId, amount: U128);
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn account_book_callback_deposit(&mut self, sender_id: AccountId, raft_id: AccountId,
                                     amount: Balance, raft_amount: Balance, user_raft_amount: Balance);

    fn account_book_callback_withdraw(&mut self, sender_id: AccountId, raft_id: AccountId,
                                      amount: Balance, raft_amount: Balance, user_raft_amount: Balance);

    fn mint_callback(&mut self, sender_id: AccountId, token_id: AccountId, token_amount: Balance,
                     raft_id: AccountId, raft_amount: Balance, join_debtpool: bool);
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
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id: owner_id.clone(),
            state: RunningState::Running,
            leverage_ratio: (1, 10),
            interest_fee: 0,
            exchange_fee: 3,
            accounts: LookupMap::new(StorageKey::Accounts),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            token_list: UnorderedMap::new(b"r".to_vec()),
            whitelisted_rafts: UnorderedSet::new(StorageKey::Whitelist),
            raft_list: UnorderedMap::new(b"r".to_vec()),
            collaterals: Vector::new(b"r".to_vec()),
            user_collaterals: LookupMap::new(b"r".to_vec()),
            debt_pool: debtpool::DebtPool::new(),
            account_book: accountbook::AccountBook::new(),
            price_oracle: oracle::PriceInfo::new(),
        }
    }

    #[payable]
    pub fn mint(&mut self, token: AccountId, token_amount: Balance,
                raft: AccountId, raft_amount: Balance, join_debtpool: bool) -> Promise {
        assert_one_yocto();
        self.assert_contract_running();

        assert!(self.is_in_whitelisted_tokens(&token));
        assert!(self.is_in_whitelisted_rafts(&raft));

        assert!(token_amount > 0, "{}", errors::NO_ATTACHED_DEPOSIT);
        assert!(raft_amount > 0, "{}", errors::SYNTHETIC_AMOUNT_ERROR);

        let sender_id = env::predecessor_account_id();
        ext_fungible_token::ft_transfer_call(
            env::current_account_id(),
            U128(token_amount),
            None,
            "".to_string(),
            token.clone(),
            utils::ONE_YOCTO,
            utils::GAS_FOR_FT_TRANSFER,
        ).then(ext_self::mint_callback(
            sender_id,
            token,
            token_amount,
            raft,
            raft_amount,
            join_debtpool,
            env::current_account_id(),
            utils::NO_DEPOSIT,
            utils::GAS_FOR_FT_TRANSFER,
        ))
    }

    #[private]
    fn mint_callback(&mut self, sender_id: AccountId, token: AccountId, token_amount: Balance,
                     raft: AccountId, raft_amount: Balance, join_debtpool: bool) {
        if join_debtpool {
            let token_decimals = self.query_token(&token).unwrap().decimals;
            let raft_decimals = self.query_raft(&raft).unwrap().decimals;

            let leverage_ratio = (self.price_oracle.get_price(&raft) * raft_amount * 10u128.pow(token_decimals))
                / (self.price_oracle.get_price(&token) * token_amount * 10u128.pow(raft_decimals));

            let (min, max) = self.leverage_ratio;
            assert!(leverage_ratio >= min.into());
            assert!(leverage_ratio <= max.into());

            self.debt_pool.join(&self.price_oracle, &sender_id, &raft, raft_amount);
        } else {
            let token_asset = self.query_token(&token).unwrap();
            let raft_asset = self.query_token(&raft).unwrap();

            let token_decimals = token_asset.decimals;
            let raft_decimals = raft_asset.decimals;

            let collateral_ratio = (self.price_oracle.get_price(&token) * token_amount * 10u128.pow(raft_decimals) * 100)
                / (self.price_oracle.get_price(&raft) * raft_amount * 10u128.pow(token_decimals));

            assert!(collateral_ratio >= token_asset.collateral_ratio);

            self.account_book.mint(&sender_id, &raft, raft_amount);
        }

        let collateral = Collateral {
            issuer: sender_id,
            token: token.clone(),
            token_amount,
            raft: raft.clone(),
            raft_amount,
            join_debtpool,
            block_index: env::block_height(),
            create_time: env::block_timestamp(),
            state: 0,
        };

        self.collaterals.push(&collateral);
    }

    pub fn swap_in_debtpool(&mut self, old_raft: AccountId, new_raft: AccountId, swap_amount: Balance) {
        self.assert_contract_running();

        assert!(self.is_in_whitelisted_rafts(&old_raft));
        assert!(self.is_in_whitelisted_rafts(&new_raft));
        assert!(swap_amount > 0);

        let sender_id = env::predecessor_account_id();

        let old_raft_amount = self.debt_pool.query_raft_amount(&old_raft);
        let old_user_raft_amount = self.debt_pool.query_user_raft_amount(&sender_id, &old_raft);
        assert!(old_user_raft_amount >= swap_amount);

        // charge transaction fee
        let exchange_fee_amount = swap_amount * self.exchange_fee as u128 / utils::FEE_DIVISOR as u128;
        let owner_raft_amount = self.debt_pool.query_user_raft_amount(&self.owner_id, &old_raft);
        self.debt_pool.insert_user_raft_amount(&self.owner_id, &old_raft, owner_raft_amount + exchange_fee_amount);

        self.debt_pool.calc_sub_raft_amount(&old_raft, &old_raft_amount, swap_amount - exchange_fee_amount);
        self.debt_pool.insert_user_raft_amount(&sender_id, &old_raft, old_user_raft_amount - swap_amount);

        let new_swap_amount = self.debt_pool.calc_raft_value(&self.price_oracle, &old_raft, swap_amount - exchange_fee_amount)
            / self.price_oracle.get_price(&new_raft);
        let new_raft_amount = self.debt_pool.query_raft_amount(&new_raft);
        self.debt_pool.calc_add_raft_amount(&new_raft, &new_raft_amount, new_swap_amount);

        let new_user_raft_amount = self.debt_pool.query_user_raft_amount(&sender_id, &new_raft);
        self.debt_pool.insert_user_raft_amount(&sender_id, &new_raft, new_user_raft_amount + new_swap_amount);
    }

    pub fn swap_in_accountbook(&mut self, old_raft: AccountId, new_raft: AccountId, swap_amount: Balance) {
        self.assert_contract_running();

        assert!(self.is_in_whitelisted_rafts(&old_raft));
        assert!(self.is_in_whitelisted_rafts(&new_raft));
        assert!(swap_amount > 0);

        let sender_id = env::predecessor_account_id();

        let old_raft_amount = self.account_book.query_raft_amount(&old_raft);
        assert!(old_raft_amount >= swap_amount);
        let old_user_raft_amount = self.account_book.query_user_raft_amount(&sender_id, &old_raft);
        assert!(old_user_raft_amount >= swap_amount);

        // charge transaction fee
        let exchange_fee_amount = swap_amount * self.exchange_fee as u128 / utils::FEE_DIVISOR as u128;
        let owner_raft_amount = self.account_book.query_user_raft_amount(&self.owner_id, &old_raft);
        self.account_book.insert_user_raft_amount(&self.owner_id, &old_raft, owner_raft_amount + exchange_fee_amount);

        // processing in the account book
        self.account_book.insert_raft_amount(&old_raft, old_raft_amount - swap_amount + exchange_fee_amount);
        self.account_book.insert_user_raft_amount(&sender_id, &old_raft, old_user_raft_amount - swap_amount);

        let new_swap_amount = self.price_oracle.get_price(&old_raft) * (swap_amount - exchange_fee_amount)
            / self.price_oracle.get_price(&new_raft);
        let new_raft_amount = self.account_book.query_raft_amount(&new_raft);
        self.account_book.insert_raft_amount(&new_raft, new_raft_amount + new_swap_amount);

        let new_user_raft_amount = self.account_book.query_user_raft_amount(&sender_id, &new_raft);
        self.account_book.insert_user_raft_amount(&sender_id, &new_raft, new_user_raft_amount + new_swap_amount);

        // processing in the debt pool
        let old_raft_amount = self.debt_pool.query_raft_amount(&old_raft);
        self.debt_pool.calc_sub_raft_amount(&old_raft, &old_raft_amount, new_swap_amount);

        let new_raft_amount = self.debt_pool.query_raft_amount(&new_raft);
        self.debt_pool.calc_add_raft_amount(&new_raft, &new_raft_amount, new_swap_amount);
    }

    #[payable]
    pub fn redeem_in_debtpool(&mut self) -> PromiseOrValue<U128> {
        assert_one_yocto();
        self.assert_contract_running();

        let opt_rusd = self.query_rusd();
        assert!(opt_rusd.is_some());
        let rusd_asset = opt_rusd.unwrap();

        let sender_id = env::predecessor_account_id();
        let collateral_ids: Option<Vector<CollateralId>> = self.user_collaterals.get(&sender_id);
        assert!(collateral_ids.is_some());

        // calculate user debt
        let user_debt_ratio = self.debt_pool.query_debt_ratio(&sender_id);
        let raft_total_value = self.debt_pool.calc_raft_total_value(&self.price_oracle);
        let user_debt = raft_total_value * user_debt_ratio / utils::RATIO_DIVISOR;

        if user_debt > 0 {
            let user_rusd_amount_in_debtpool = self.debt_pool.query_user_raft_amount(&sender_id, &rusd_asset.address);
            let user_debt_amount = user_debt / utils::PRICE_PRECISION as u128;
            if user_debt <= user_rusd_amount_in_debtpool * utils::PRICE_PRECISION as u128 {
                // subtract user raft amount
                self.debt_pool.insert_user_raft_amount(&sender_id, &rusd_asset.address, user_rusd_amount_in_debtpool - user_debt_amount);

                // subtract total raft amount
                let rusd_amount = self.debt_pool.query_raft_amount(&rusd_asset.address);
                self.debt_pool.calc_sub_raft_amount(&rusd_asset.address, &rusd_amount, user_debt_amount);

                // remove user debt ratio
                self.debt_pool.remove_debt_ratio(&sender_id);
            } else {
                let user_rusd_amount_in_accountbook = self.account_book.query_user_raft_amount(&sender_id, &rusd_asset.address);
                assert!(user_debt_amount <= user_rusd_amount_in_debtpool + user_rusd_amount_in_accountbook);

                // remove user raft amount in debt pool
                self.debt_pool.remove_user_raft_amount(&sender_id, &rusd_asset.address);

                // subtract total raft amount in debt pool
                let rusd_amount_in_debtpool = self.debt_pool.query_raft_amount(&rusd_asset.address);
                self.debt_pool.calc_sub_raft_amount(&rusd_asset.address, &rusd_amount_in_debtpool,
                                                    user_rusd_amount_in_debtpool);

                // remove user debt ratio
                self.debt_pool.remove_debt_ratio(&sender_id);

                let remaining_debt_amount = user_debt_amount - user_rusd_amount_in_debtpool;
                // subtract user raft amount in account book
                self.account_book.insert_user_raft_amount(&sender_id, &rusd_asset.address, user_rusd_amount_in_accountbook - remaining_debt_amount);

                // subtract total raft amount in account book
                let rusd_amount_in_accountbook = self.account_book.query_raft_amount(&rusd_asset.address);
                self.account_book.insert_raft_amount(&rusd_asset.address, rusd_amount_in_accountbook - remaining_debt_amount);
            }
        }

        // transfer debt pool assets to account book
        for (raft, amount) in self.debt_pool.query_user_raft_amounts(&sender_id).iter() {
            let debtpool_raft_amount = self.debt_pool.query_raft_amount(raft);
            self.debt_pool.calc_sub_raft_amount(raft, &debtpool_raft_amount, *amount);
            self.debt_pool.remove_user_raft_amount(&sender_id, raft);

            let accountbook_raft_amount = self.account_book.query_raft_amount(raft);
            self.account_book.insert_raft_amount(raft, accountbook_raft_amount + amount);

            let accountbook_user_raft_amount = self.account_book.query_user_raft_amount(&sender_id, raft);
            self.account_book.insert_user_raft_amount(&sender_id, raft, accountbook_user_raft_amount + amount);
        }

        // recalculating debt ratio
        let new_raft_total_value = self.debt_pool.calc_raft_total_value(&self.price_oracle);
        self.debt_pool.calc_all_debt_ratio(raft_total_value, new_raft_total_value);

        // return of collateral assets
        for collateral_id in collateral_ids.unwrap().iter() {
            let opt_collateral = self.query_collateral(collateral_id);
            if opt_collateral.is_none() { continue; }
            let collateral = opt_collateral.unwrap();

            let mut account = self.internal_unwrap_account(&sender_id);
            account.withdraw(&collateral.token, collateral.token_amount);
            self.internal_save_account(&sender_id, account);
            self.internal_send_tokens(&sender_id, &collateral.token, collateral.token_amount);
        }

        PromiseOrValue::Value(U128(0))
    }

    #[payable]
    pub fn redeem_in_accountbook(&mut self, collateral_id: CollateralId) -> Promise {
        assert_one_yocto();
        self.assert_contract_running();

        let opt_collateral = self.query_collateral(collateral_id);
        assert!(opt_collateral.is_some());

        let sender_id = env::predecessor_account_id();
        let collateral = opt_collateral.unwrap();
        assert_eq!(collateral.issuer, sender_id);
        assert_eq!(collateral.join_debtpool, false);
        assert_eq!(collateral.state, 0);

        let raft_amount = self.account_book.query_raft_amount(&collateral.raft);
        let user_raft_amount = self.account_book.query_user_raft_amount(&sender_id, &collateral.raft);
        let interest_fee_amount = collateral.raft_amount * self.interest_fee as u128 / utils::FEE_DIVISOR as u128;
        assert!(raft_amount > collateral.raft_amount + interest_fee_amount);
        assert!(user_raft_amount > collateral.raft_amount + interest_fee_amount);

        // charge interest fee
        let owner_raft_amount = self.account_book.query_user_raft_amount(&self.owner_id, &collateral.raft);
        self.account_book.insert_user_raft_amount(&self.owner_id, &collateral.raft, owner_raft_amount + interest_fee_amount);

        // subtract user raft amount
        self.account_book.insert_user_raft_amount(&sender_id, &collateral.raft, user_raft_amount - collateral.raft_amount - interest_fee_amount);

        // subtract total raft amount
        self.account_book.insert_raft_amount(&collateral.raft, raft_amount - collateral.raft_amount);

        let mut account = self.internal_unwrap_account(&sender_id);
        account.withdraw(&collateral.token, collateral.token_amount);
        self.internal_save_account(&sender_id, account);
        self.internal_send_tokens(&sender_id, &collateral.token, collateral.token_amount)
    }

    #[payable]
    pub fn deposit_in_accountbook(&mut self, raft_id: AccountId, amount: Balance) -> Promise {
        assert_one_yocto();
        self.assert_contract_running();

        let sender_id = env::predecessor_account_id();
        let raft_amount = self.account_book.query_raft_amount(&raft_id);
        let user_raft_amount = self.account_book.query_user_raft_amount(&sender_id, &raft_id);

        ext_enhanced_fungible_token::burn(
            sender_id.clone(),
            U128(amount),
            raft_id.clone(),
            utils::ONE_YOCTO,
            utils::GAS_FOR_FT_TRANSFER,
        ).then(ext_self::account_book_callback_deposit(
            sender_id.clone(),
            raft_id.clone(),
            amount,
            raft_amount,
            user_raft_amount,
            env::current_account_id(),
            utils::NO_DEPOSIT,
            utils::GAS_FOR_FT_TRANSFER,
        ))
    }

    #[payable]
    pub fn withdraw_in_accountbook(&mut self, raft_id: AccountId, amount: Balance) -> Promise {
        assert_one_yocto();
        self.assert_contract_running();

        assert!(amount > 0, "{}", errors::ILLEGAL_WITHDRAW_AMOUNT);

        let sender_id = env::predecessor_account_id();
        let raft_amount = self.account_book.query_raft_amount(&raft_id);
        let user_raft_amount = self.account_book.query_user_raft_amount(&sender_id, &raft_id);
        assert!(raft_amount >= amount);
        assert!(user_raft_amount >= amount);

        ext_enhanced_fungible_token::mint(
            sender_id.clone(),
            U128(amount),
            raft_id.clone(),
            utils::ONE_YOCTO,
            utils::GAS_FOR_FT_TRANSFER,
        ).then(ext_self::account_book_callback_withdraw(
            sender_id.clone(),
            raft_id.clone(),
            amount,
            raft_amount,
            user_raft_amount,
            env::current_account_id(),
            utils::NO_DEPOSIT,
            utils::GAS_FOR_FT_TRANSFER,
        ))
    }
}

/// Internal methods implementation.
impl Contract {
    fn assert_contract_running(&self) {
        match self.state {
            RunningState::Running => (),
            _ => env::panic_str(errors::CONTRACT_PAUSED),
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

        assert_eq!(user, env::predecessor_account_id(), "{}", errors::NO_PERMISSION);
    }
}
