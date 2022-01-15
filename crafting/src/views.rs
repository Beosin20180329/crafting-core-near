use crate::*;

#[near_bindgen]
impl Contract {
    /// Token list Related
    pub fn get_token(&self, token: ValidAccountId) -> Option<Asset> {
        self.query_token(token.as_ref().clone())
    }

    pub fn whitelisted_tokens(&self) -> Vec<Asset> {
        let mut vec: Vec<Asset> = Vec::new();
        for account_id in self.whitelisted_tokens.iter() {
            let asset = self.query_token(account_id);
            if asset.is_some() {
                vec.push(asset.unwrap());
            }
        }

        vec
    }

    /// Raft list Related
    pub fn get_raft(&self, raft: ValidAccountId) -> Option<Asset> {
        self.query_raft(raft.as_ref().clone())
    }

    pub fn whitelisted_rafts(&self) -> Vec<Asset> {
        let mut vec: Vec<Asset> = Vec::new();
        for account_id in self.whitelisted_rafts.iter() {
            let asset = self.query_raft(account_id);
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

    pub fn user_collaterals(&self, user: ValidAccountId) -> Vec<Collateral> {
        self.assert_query_authority(user.as_ref().clone());

        let mut vec: Vec<Collateral> = Vec::new();
        let collateral_ids: Option<Vector<CollateralId>> = self.user_collaterals.get(user.as_ref());
        if collateral_ids.is_none() {
            return vec;
        }

        for collateral_id in collateral_ids.unwrap().iter() {
            let collateral = self.query_collateral(collateral_id);
            if collateral.is_some() {
                vec.push(collateral.unwrap());
            }
        }

        vec
    }

    /// Debt Pool Related
    pub fn debtpool_raft_amount(&self, raft: ValidAccountId) -> Balance {
        self.is_in_whitelisted_rafts(raft.as_ref().clone());

        self.debt_pool.query_raft_amount(raft.as_ref().clone())
    }

    pub fn debtpool_raft_value(&self, raft: ValidAccountId) -> (Balance, u128) {
        let amount = self.debtpool_raft_amount(raft.clone());
        let value = self.debt_pool.calc_raft_value(&self.price_oracle, raft.as_ref().clone(), amount);
        (amount, value)
    }

    pub fn debtpool_raft_total_value(&self) -> u128 {
        self.debt_pool.calc_raft_total_value(&self.price_oracle)
    }

    pub fn debtpool_user_raft_amount(&self, user: ValidAccountId, raft: ValidAccountId) -> Balance {
        self.assert_query_authority(user.as_ref().clone());
        self.is_in_whitelisted_rafts(raft.as_ref().clone());

        self.debt_pool.query_user_raft_amount(user.as_ref().clone(), raft.as_ref().clone())
    }

    pub fn debtpool_user_raft_value(&self, user: ValidAccountId, raft: ValidAccountId) -> (Balance, u128) {
        let amount = self.debtpool_user_raft_amount(user.clone(), raft.clone());
        let value = self.debt_pool.calc_raft_value(&self.price_oracle, raft.as_ref().clone(), amount);
        (amount, value)
    }

    pub fn debtpool_user_raft_total_value(&self, user: ValidAccountId) -> u128 {
        self.assert_query_authority(user.as_ref().clone());

        self.debt_pool.calc_user_raft_total_value(&self.price_oracle, user.as_ref().clone())
    }

    pub fn debtpool_user_profit(&self, user: ValidAccountId) -> i128 {
        self.assert_query_authority(user.as_ref().clone());

        (self.debt_pool.calc_user_raft_total_value(&self.price_oracle, user.as_ref().clone()) -
            (self.debtpool_raft_total_value() * self.debtpool_debt_ratio(user)) / utils::RATIO_DIVISOR) as i128
    }

    pub fn debtpool_debt_ratio(&self, user: ValidAccountId) -> u128 {
        self.assert_query_authority(user.as_ref().clone());

        self.debt_pool.query_debt_ratio(user.as_ref().clone())
    }

    /// AccountBook Related
    pub fn accountbook_raft_amount(&self, raft: ValidAccountId) -> Balance {
        self.is_in_whitelisted_rafts(raft.as_ref().clone());

        self.account_book.query_raft_amount(raft.as_ref().clone())
    }

    pub fn accountbook_raft_value(&self, raft: ValidAccountId) -> (Balance, u128) {
        let amount = self.accountbook_raft_amount(raft.clone());
        let value = self.account_book.calc_raft_value(&self.price_oracle, raft.as_ref().clone(), amount);
        (amount, value)
    }

    pub fn accountbook_raft_total_value(&self) -> u128 {
        self.account_book.calc_raft_total_value(&self.price_oracle)
    }

    pub fn accountbook_user_raft_amount(&self, user: ValidAccountId, raft: ValidAccountId) -> Balance {
        self.assert_query_authority(user.as_ref().clone());
        self.is_in_whitelisted_rafts(raft.as_ref().clone());

        self.account_book.query_user_raft_amount(user.as_ref().clone(), raft.as_ref().clone())
    }

    pub fn accountbook_user_raft_value(&self, user: ValidAccountId, raft: ValidAccountId) -> (Balance, u128) {
        let amount = self.accountbook_user_raft_amount(user.clone(), raft.clone());
        let value = self.account_book.calc_raft_value(&self.price_oracle, raft.as_ref().clone(), amount);
        (amount, value)
    }

    pub fn accountbook_user_raft_total_value(&self, user: ValidAccountId) -> u128 {
        self.assert_query_authority(user.as_ref().clone());

        self.account_book.calc_user_raft_total_value(&self.price_oracle, user.as_ref().clone())
    }

    /// Owner Related
    pub fn contract_owner(&self) -> AccountId {
        self.owner_id.clone()
    }
}
