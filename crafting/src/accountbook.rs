use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::{AccountId, Balance};

use crate::*;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountBook {
    /// Mapping from raft to amount of raft that is in debt pool.
    raft_amounts: UnorderedMap<AccountId, Balance>,
    /// Mapping from user and raft to amount of raft that is in debt pool.
    user_raft_amounts: LookupMap<(AccountId, AccountId), Balance>,
}

impl AccountBook {
    pub(crate) fn new() -> Self {
        Self {
            raft_amounts: UnorderedMap::new(b"r".to_vec()),
            user_raft_amounts: LookupMap::new(b"r".to_vec()),
        }
    }

    pub(crate) fn mint(&mut self, user: &AccountId, raft_id: &AccountId, raft_amount: Balance) {
        let old_amount = self.query_raft_amount(raft_id);
        self.insert_raft_amount(raft_id, old_amount + raft_amount);

        let old_amount = self.query_user_raft_amount(user, raft_id);
        self.insert_user_raft_amount(user, raft_id, old_amount + raft_amount);
    }

    pub(crate) fn query_raft_amount(&self, raft_id: &AccountId) -> Balance {
        self.raft_amounts.get(raft_id).unwrap_or(0)
    }

    pub(crate) fn insert_raft_amount(&mut self, raft_id: &AccountId, amount: Balance) {
        self.raft_amounts.insert(raft_id, &amount);
    }

    pub(crate) fn query_user_raft_amount(&self, user: &AccountId, raft_id: &AccountId) -> Balance {
        self.user_raft_amounts.get(&(user.clone(), raft_id.clone())).unwrap_or(0)
    }

    pub(crate) fn insert_user_raft_amount(&mut self, user: &AccountId, raft_id: &AccountId, amount: Balance) {
        self.user_raft_amounts.insert(&(user.clone(), raft_id.clone()), &amount);
    }

    pub(crate) fn calc_raft_value(&self, price_oracle: &oracle::PriceInfo, raft_id: &AccountId, amount: Balance) -> u128 {
        price_oracle.get_price(raft_id) * amount
    }

    pub(crate) fn calc_raft_total_value(&self, price_oracle: &oracle::PriceInfo) -> u128 {
        let mut total: u128 = 0;
        for (raft, amount) in self.raft_amounts.iter() {
            total += self.calc_raft_value(price_oracle, &raft, amount);
        }

        total
    }

    pub(crate) fn calc_user_raft_total_value(&self, price_oracle: &oracle::PriceInfo, user: &AccountId) -> u128 {
        let mut total: u128 = 0;
        for (raft, _) in self.raft_amounts.iter() {
            let amount = self.query_user_raft_amount(user, &raft);
            if amount != 0 {
                total += self.calc_raft_value(price_oracle, &raft, amount);
            }
        }

        total
    }
}

#[near_bindgen]
impl Contract {
    #[private]
    pub fn account_book_callback_deposit(&mut self, sender_id: AccountId, raft_id: AccountId,
                                         amount: Balance, raft_amount: Balance, user_raft_amount: Balance) {
        self.account_book.insert_raft_amount(&raft_id, raft_amount + amount);
        self.account_book.insert_user_raft_amount(&sender_id, &raft_id, user_raft_amount + amount);
    }

    #[private]
    pub fn account_book_callback_withdraw(&mut self, sender_id: AccountId, raft_id: AccountId,
                                          amount: Balance, raft_amount: Balance, user_raft_amount: Balance) {
        self.account_book.insert_raft_amount(&raft_id, raft_amount - amount);
        self.account_book.insert_user_raft_amount(&sender_id, &raft_id, user_raft_amount - amount);
    }
}
