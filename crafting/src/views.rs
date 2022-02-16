use crate::*;
use crate::debtpool::WrappedBalance;

#[near_bindgen]
impl Contract {
    /// Token list Related
    pub fn get_token(&self, token_id: AccountId) -> Option<Asset> {
        self.query_token(&token_id)
    }

    pub fn whitelisted_tokens(&self) -> Vec<Asset> {
        let mut vec: Vec<Asset> = Vec::new();
        for account_id in self.whitelisted_tokens.iter() {
            let asset = self.query_token(&account_id);
            if asset.is_some() {
                vec.push(asset.unwrap());
            }
        }

        vec
    }

    /// Raft list Related
    pub fn get_raft(&self, raft_id: AccountId) -> Option<Asset> {
        self.query_raft(&raft_id)
    }

    pub fn whitelisted_rafts(&self) -> Vec<Asset> {
        let mut vec: Vec<Asset> = Vec::new();
        for account_id in self.whitelisted_rafts.iter() {
            let asset = self.query_raft(&account_id);
            if asset.is_some() {
                vec.push(asset.unwrap());
            }
        }

        vec
    }

    /// Collateral Related
    pub fn collateral_count(&self) -> CollateralId {
        self.collaterals.len()
    }

    pub fn get_collateral(&self, collateral_id: CollateralId) -> Option<Collateral> {
        self.query_collateral(collateral_id)
    }

    pub fn user_collaterals(&self, user: AccountId) -> Vec<Collateral> {
        self.assert_query_authority(user.clone());

        let mut vec: Vec<Collateral> = Vec::new();
        let collateral_ids: Option<Vector<CollateralId>> = self.user_collaterals.get(&user);
        if collateral_ids.is_none() {
            return vec;
        }

        for collateral_id in collateral_ids.unwrap().iter() {
            let opt_collateral = self.query_collateral(collateral_id);
            if opt_collateral.is_some() {
                vec.push(opt_collateral.unwrap());
            }
        }

        vec
    }

    /// Debt Pool Related
    pub fn debtpool_raft_amount(&self, raft_id: AccountId) -> WrappedBalance {
        self.is_in_whitelisted_rafts(&raft_id);

        self.debt_pool.query_raft_amount(&raft_id)
    }

    pub fn debtpool_raft_value(&self, raft_id: AccountId) -> (WrappedBalance, u128) {
        let raft_amount = self.debtpool_raft_amount(raft_id.clone());
        let value = self.debt_pool.calc_raft_value(&self.price_oracle, &raft_id, raft_amount.amount);
        (raft_amount, value)
    }

    pub fn debtpool_raft_total_value(&self) -> u128 {
        self.debt_pool.calc_raft_total_value(&self.price_oracle)
    }

    pub fn debtpool_user_raft_amount(&self, user: AccountId, raft_id: AccountId) -> Balance {
        self.assert_query_authority(user.clone());
        self.is_in_whitelisted_rafts(&raft_id);

        self.debt_pool.query_user_raft_amount(&user, &raft_id)
    }

    pub fn debtpool_user_raft_value(&self, user: AccountId, raft_id: AccountId) -> (Balance, u128) {
        let amount = self.debtpool_user_raft_amount(user.clone(), raft_id.clone());
        let value = self.debt_pool.calc_raft_value(&self.price_oracle, &raft_id, amount);
        (amount, value)
    }

    pub fn debtpool_user_raft_total_value(&self, user: AccountId) -> u128 {
        self.assert_query_authority(user.clone());

        self.debt_pool.calc_user_raft_total_value(&self.price_oracle, &user)
    }

    pub fn debtpool_user_profit(&self, user: AccountId) -> i128 {
        self.assert_query_authority(user.clone());

        (self.debt_pool.calc_user_raft_total_value(&self.price_oracle, &user) -
            (self.debtpool_raft_total_value() * self.debtpool_debt_ratio(user)) / utils::RATIO_DIVISOR) as i128
    }

    pub fn debtpool_debt_ratio(&self, user: AccountId) -> u128 {
        self.assert_query_authority(user.clone());

        self.debt_pool.query_debt_ratio(&user)
    }

    /// AccountBook Related
    pub fn accountbook_raft_amount(&self, raft_id: AccountId) -> Balance {
        self.is_in_whitelisted_rafts(&raft_id);

        self.account_book.query_raft_amount(&raft_id)
    }

    pub fn accountbook_raft_value(&self, raft_id: AccountId) -> (Balance, u128) {
        let amount = self.accountbook_raft_amount(raft_id.clone());
        let value = self.account_book.calc_raft_value(&self.price_oracle, &raft_id, amount);
        (amount, value)
    }

    pub fn accountbook_raft_total_value(&self) -> u128 {
        self.account_book.calc_raft_total_value(&self.price_oracle)
    }

    pub fn accountbook_user_raft_amount(&self, user: AccountId, raft_id: AccountId) -> Balance {
        self.assert_query_authority(user.clone());
        self.is_in_whitelisted_rafts(&raft_id);

        self.account_book.query_user_raft_amount(&user, &raft_id)
    }

    pub fn accountbook_user_raft_value(&self, user: AccountId, raft_id: AccountId) -> (Balance, u128) {
        let amount = self.accountbook_user_raft_amount(user.clone(), raft_id.clone());
        let value = self.account_book.calc_raft_value(&self.price_oracle, &raft_id, amount);
        (amount, value)
    }

    pub fn accountbook_user_raft_total_value(&self, user: AccountId) -> u128 {
        self.assert_query_authority(user.clone());

        self.account_book.calc_user_raft_total_value(&self.price_oracle, &user)
    }

    /// Owner Related
    pub fn contract_owner(&self) -> AccountId {
        self.owner_id.clone()
    }
}
