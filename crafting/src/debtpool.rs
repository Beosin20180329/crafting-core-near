use std::collections::HashMap;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::{AccountId, Balance};

use crate::*;

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct WrappedBalance {
    pub(crate) amount: Balance,
    pub(crate) is_positive: bool,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct DebtPool {
    /// Mapping from raft to amount of raft that is in debt pool.
    raft_amounts: UnorderedMap<AccountId, WrappedBalance>,
    /// Mapping from user and raft to amount of raft that is in debt pool.
    user_raft_amounts: LookupMap<(AccountId, AccountId), Balance>,
    /// Mapping from user to debt ratio.
    debt_ratios: HashMap<AccountId, u128>,
}

impl DebtPool {
    pub(crate) fn new() -> Self {
        Self {
            raft_amounts: UnorderedMap::new(b"r".to_vec()),
            user_raft_amounts: LookupMap::new(b"r".to_vec()),
            debt_ratios: HashMap::new(),
        }
    }

    pub(crate) fn join(&mut self, price_oracle: &oracle::PriceInfo, user: &AccountId,
                       raft_id: &AccountId, raft_amount: Balance) {
        if self.raft_amounts.is_empty() {
            self.insert_raft_amount(raft_id, &WrappedBalance {
                amount: raft_amount,
                is_positive: true,
            });
            self.insert_user_raft_amount(user, raft_id, raft_amount);
            self.insert_debt_ratio(user.clone(), utils::RATIO_DIVISOR);
        } else {
            let old_total_value = self.calc_raft_total_value(price_oracle);

            let old_raft_amount = self.query_raft_amount(raft_id);
            self.calc_add_raft_amount(raft_id, &old_raft_amount, raft_amount);

            let old_user_raft_amount = self.query_user_raft_amount(user, raft_id);
            self.insert_user_raft_amount(user, raft_id, old_user_raft_amount + raft_amount);

            let join_raft_value = self.calc_raft_value(price_oracle, raft_id, raft_amount);
            let new_total_value = old_total_value + join_raft_value;

            self.calc_debt_ratio(old_total_value, new_total_value, user.clone());
        }
    }

    pub(crate) fn query_raft_amount(&self, raft_id: &AccountId) -> WrappedBalance {
        let opt_wbalance = self.raft_amounts.get(raft_id);
        if opt_wbalance.is_some() {
            opt_wbalance.unwrap();
        }

        return WrappedBalance{
            amount: 0,
            is_positive: true,
        }
    }

    pub(crate) fn insert_raft_amount(&mut self, raft_id: &AccountId, amount: &WrappedBalance) {
        self.raft_amounts.insert(raft_id, amount);
    }

    pub(crate) fn query_user_raft_amount(&self, user: &AccountId, raft_id: &AccountId) -> Balance {
        self.user_raft_amounts.get(&(user.clone(), raft_id.clone())).unwrap_or(0)
    }

    pub(crate) fn query_user_raft_amounts(&self, user: &AccountId) -> Vec<(AccountId, Balance)> {
        let mut vec: Vec<(AccountId, Balance)> = Vec::new();
        for (raft_id, _) in self.raft_amounts.iter() {
            let amount = self.query_user_raft_amount(user, &raft_id);
            if amount != 0 {
                vec.push( (raft_id, amount));
            }
        }

        vec
    }

    pub(crate) fn insert_user_raft_amount(&mut self, user: &AccountId, raft_id: &AccountId, amount: Balance) {
        self.user_raft_amounts.insert(&(user.clone(), raft_id.clone()), &amount);
    }

    pub(crate) fn remove_user_raft_amount(&mut self, user: &AccountId, raft_id: &AccountId) {
        self.user_raft_amounts.remove(&(user.clone(), raft_id.clone()));
    }

    pub(crate) fn calc_raft_value(&self, price_oracle: &oracle::PriceInfo, raft_id: &AccountId, amount: Balance) -> u128 {
        price_oracle.get_price(raft_id) * amount
    }

    pub(crate) fn query_debt_ratio(&self, user: &AccountId) -> u128 {
        self.debt_ratios.get(user).copied().unwrap_or(0)
    }

    fn insert_debt_ratio(&mut self, user: AccountId, debt_ratio: u128) {
        self.debt_ratios.insert(user, debt_ratio);
    }

    pub(crate) fn remove_debt_ratio(&mut self, user: &AccountId) {
        self.debt_ratios.remove(user);
    }

    pub(crate) fn calc_raft_total_value(&self, price_oracle: &oracle::PriceInfo) -> u128 {
        let mut total: u128 = 0;
        for (raft, wbalance) in self.raft_amounts.iter() {
            total += self.calc_raft_value(price_oracle, &raft, wbalance.amount);
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

    /// Calculate the debt ratio.
    fn calc_debt_ratio(&mut self, old_total_value: u128, new_total_value: u128, sender_id: AccountId) {
        if new_total_value == 0 { return; }

        let mut is_new_user = true;

        for (user, debt_ratio) in self.debt_ratios.iter_mut() {
            if *user != sender_id {
                *debt_ratio = (old_total_value * (*debt_ratio)) / new_total_value;
            } else {
                *debt_ratio = (old_total_value * (*debt_ratio) + (new_total_value - old_total_value) * utils::RATIO_DIVISOR) / new_total_value;
                is_new_user = false;
            }
        }

        if is_new_user {
            self.insert_debt_ratio(sender_id, (new_total_value - old_total_value) * utils::RATIO_DIVISOR / new_total_value);
        }
    }

    pub (crate) fn calc_all_debt_ratio(&mut self, old_total_value: u128, new_total_value: u128) {
        if new_total_value == 0 { return; }

        for (_, debt_ratio) in self.debt_ratios.iter_mut() {
            *debt_ratio = (old_total_value * (*debt_ratio)) / new_total_value;
        }
    }

    pub(crate) fn calc_add_raft_amount(&mut self, raft_id: &AccountId, raft_amount: &WrappedBalance, amount: Balance) {
        if raft_amount.is_positive {
            self.insert_raft_amount(raft_id, &WrappedBalance {
                amount: raft_amount.amount + amount,
                is_positive: true,
            });
        } else {
            if raft_amount.amount >= amount {
                self.insert_raft_amount(raft_id, &WrappedBalance {
                    amount: raft_amount.amount - amount,
                    is_positive: false,
                });
            } else {
                self.insert_raft_amount(raft_id, &WrappedBalance {
                    amount: amount - raft_amount.amount,
                    is_positive: true,
                });
            }
        }
    }

    pub(crate) fn calc_sub_raft_amount(&mut self, raft_id: &AccountId, raft_amount: &WrappedBalance, amount: Balance) {
        if raft_amount.is_positive {
            if raft_amount.amount >= amount {
                self.insert_raft_amount(raft_id, &WrappedBalance {
                    amount: raft_amount.amount - amount,
                    is_positive: true,
                });
            } else {
                self.insert_raft_amount(raft_id, &WrappedBalance {
                    amount: amount - raft_amount.amount,
                    is_positive: false,
                });
            }
        } else {
            self.insert_raft_amount(raft_id, &WrappedBalance {
                amount: raft_amount.amount + amount,
                is_positive: false,
            });
        }
    }
}
