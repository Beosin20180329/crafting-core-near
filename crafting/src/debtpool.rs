use std::collections::HashMap;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::{env, AccountId, Balance};

use crate::*;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct DebtPool {
    /// Mapping from raft to amount of raft that is in debt pool.
    raft_amounts: UnorderedMap<AccountId, Balance>,
    /// Mapping from user and raft to amount of raft that is in debt pool.
    user_raft_amounts: LookupMap<(AccountId, AccountId), Balance>,
    /// Mapping from user to debt ratio.
    debt_ratios: HashMap<AccountId, u128>,
}

impl DebtPool {
    pub fn new() -> Self {
        Self {
            raft_amounts: UnorderedMap::new(b"r".to_vec()),
            user_raft_amounts: LookupMap::new(b"r".to_vec()),
            debt_ratios: HashMap::new(),
        }
    }

    pub fn join(&mut self, price_oracle: &oracle::PriceInfo, user: AccountId, raft: AccountId, raft_amount: Balance) {
        if self.raft_amounts.is_empty() {
            self.raft_amounts.insert(&raft, &raft_amount);
            self.user_raft_amounts.insert(&(user.clone(), raft), &raft_amount);
            self.debt_ratios.insert(user, utils::RATIO_DIVISOR);
        } else {
            let old_total_value = self.calc_raft_total_value(price_oracle);

            let old_amount = self.query_raft_amount(raft.clone());
            self.raft_amounts.insert(&raft, &(old_amount + raft_amount));

            let old_amount = self.query_user_raft_amount(user.clone(), raft.clone());
            self.user_raft_amounts.insert(&(user.clone(), raft.clone()), &(old_amount + raft_amount));

            let join_raft_value = self.calc_raft_value(price_oracle, raft.clone(), raft_amount);
            let new_total_value = old_total_value + join_raft_value;

            self.calc_debt_ratio(old_total_value, new_total_value, user);
        }
    }

    pub fn swap(&mut self, user: AccountId, old_raft: AccountId, new_raft: AccountId,
                swap_amount: Balance, price_oracle: &oracle::PriceInfo, owner_id: AccountId, exchange_fee: u32) {
        let old_amount = self.query_raft_amount(old_raft.clone());
        assert!(old_amount >= swap_amount);
        self.raft_amounts.insert(&old_raft, &(old_amount - swap_amount));

        // charge transaction fee
        let exchange_fee_amount = swap_amount * exchange_fee as u128 / utils::FEE_DIVISOR as u128;
        let owner_raft_amount = self.query_user_raft_amount(owner_id.clone(), old_raft.clone());
        self.user_raft_amounts.insert(&(owner_id.clone(), old_raft.clone()), &(owner_raft_amount + exchange_fee_amount));

        let new_swap_amount = self.calc_raft_value(price_oracle, old_raft.clone(), swap_amount - exchange_fee_amount)
            / price_oracle.get_price(new_raft.clone());
        let new_amount = self.query_raft_amount(new_raft.clone());
        self.raft_amounts.insert(&new_raft, &(new_amount + new_swap_amount));

        let old_amount = self.query_user_raft_amount(user.clone(), old_raft.clone());
        assert!(old_amount >= swap_amount);
        self.user_raft_amounts.insert(&(user.clone(), old_raft.clone()), &(old_amount - swap_amount));

        let new_amount = self.query_user_raft_amount(user.clone(), new_raft.clone());
        self.user_raft_amounts.insert(&(user.clone(), new_raft.clone()), &(new_amount + new_swap_amount));
    }

    pub(crate) fn query_raft_amount(&self, raft: AccountId) -> Balance {
        self.raft_amounts.get(&raft).unwrap_or(0)
    }

    pub(crate) fn query_user_raft_amount(&self, user: AccountId, raft: AccountId) -> Balance {
        self.user_raft_amounts.get(&(user, raft)).unwrap_or(0)
    }

    pub(crate) fn calc_raft_value(&self, price_oracle: &oracle::PriceInfo, raft: AccountId, amount: Balance) -> u128 {
        price_oracle.get_price(raft) * amount
    }

    pub(crate) fn query_debt_ratio(&self, user: AccountId) -> u128 {
        self.debt_ratios.get(&user).copied().unwrap_or(0)
    }

    pub(crate) fn calc_raft_total_value(&self, price_oracle: &oracle::PriceInfo) -> u128 {
        let mut total: u128 = 0;
        for (raft, amount) in self.raft_amounts.iter() {
            total += self.calc_raft_value(price_oracle, raft, amount);
        }

        total
    }

    pub(crate) fn calc_user_raft_total_value(&self, price_oracle: &oracle::PriceInfo, user: AccountId) -> u128 {
        let mut total: u128 = 0;
        for (raft, _) in self.raft_amounts.iter() {
            let amount = self.query_user_raft_amount(user.clone(), raft.clone());
            if amount != 0 {
                total += self.calc_raft_value(price_oracle, raft.clone(), amount);
            }
        }

        total
    }

    /// Calculate the debt ratio.
    fn calc_debt_ratio(&mut self, old_total_value: u128, new_total_value: u128, caller: AccountId) {
        if new_total_value == 0 { return; }

        let mut is_new_user = true;

        for (user, debt_ratio) in self.debt_ratios.iter_mut() {
            if *user != caller {
                *debt_ratio = (old_total_value * (*debt_ratio)) / new_total_value;
            } else {
                *debt_ratio = (old_total_value * (*debt_ratio) + (new_total_value - old_total_value) * utils::RATIO_DIVISOR) / new_total_value;
                is_new_user = false;
            }
        }

        if is_new_user {
            self.debt_ratios.insert(caller, (new_total_value - old_total_value) * utils::RATIO_DIVISOR / new_total_value);
        }
    }
}