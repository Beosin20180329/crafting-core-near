use crate::*;

#[near_bindgen]
impl Contract {
    /// Change owner. Only can be called by owner.
    pub fn set_owner(&mut self, owner_id: ValidAccountId) {
        self.assert_owner();
        self.owner_id = owner_id.as_ref().clone();
    }

    /// Change state of contract, Only can be called by owner or guardians.
    pub fn change_state(&mut self, state: RunningState) {
        self.assert_owner();
        if self.state != state {
            env::log(
                format!(
                    "Contract state changed from {} to {} by {}",
                    self.state, state, env::predecessor_account_id()
                ).as_bytes(),
            );
            self.state = state;
        }
    }

    /// Set leverage ratio. Only can be called by owner.
    pub fn set_leverage_ratio(&mut self, leverage_ratio: (u8, u8)) {
        self.assert_owner();
        let (min, max) = leverage_ratio;
        assert!(min >= 1);
        assert!(max <= 100);
        self.leverage_ratio = leverage_ratio;
    }

    /// Set interest fee. Only can be called by owner.
    pub fn set_interest_fee(&mut self, interest_fee: u32) {
        self.assert_owner();
        assert!(interest_fee <= utils::FEE_DIVISOR, "{}", errors::ILLEGAL_FEE);
        self.interest_fee = interest_fee;
    }

    /// Set exchange fee. Only can be called by owner.
    pub fn set_exchange_fee(&mut self, exchange_fee: u32) {
        self.assert_owner();
        assert!(exchange_fee <= utils::FEE_DIVISOR, "{}", errors::ILLEGAL_FEE);
        self.exchange_fee = exchange_fee;
    }

    /// Add whitelisted tokens with new tokens. Only can be called by owner.
    pub fn add_whitelisted_tokens(&mut self, tokens: Vec<ValidAccountId>) {
        self.assert_owner();
        for token in tokens {
            let opt = self.token_list.get(token.as_ref());
            if opt.is_some() {
                self.whitelisted_tokens.insert(token.as_ref());
            }
        }
    }

    /// Remove whitelisted token. Only can be called by owner.
    pub fn remove_whitelisted_tokens(&mut self, tokens: Vec<ValidAccountId>) {
        self.assert_owner();
        for token in tokens {
            self.whitelisted_tokens.remove(token.as_ref());
        }
    }

    /// Add token. Only can be called by owner.
    pub fn add_token_list(&mut self, name: String, symbol: String, standard: String,
                          decimals: u32, address: ValidAccountId, feed_address: ValidAccountId,
                          collateral_ratio: u128, state: u8) {
        self.assert_owner();
        let asset = Asset {
            name,
            symbol,
            standard,
            decimals,
            address: address.as_ref().clone(),
            feed_address: feed_address.as_ref().clone(),
            collateral_ratio,
            state,
        };
        self.token_list.insert(address.as_ref(), &asset);
    }

    /// Add whitelisted tokens with new rafts. Only can be called by owner.
    pub fn add_whitelisted_rafts(&mut self, rafts: Vec<ValidAccountId>) {
        self.assert_owner();
        for raft in rafts {
            let opt = self.raft_list.get(raft.as_ref());
            if opt.is_some() {
                self.whitelisted_rafts.insert(raft.as_ref());
            }
        }
    }

    /// Remove whitelisted raft. Only can be called by owner.
    pub fn remove_whitelisted_rafts(&mut self, rafts: Vec<ValidAccountId>) {
        self.assert_owner();
        for raft in rafts {
            self.whitelisted_rafts.remove(raft.as_ref());
        }
    }

    /// Add raft. Only can be called by owner.
    pub fn add_raft_list(&mut self, name: String, symbol: String, standard: String,
                          decimals: u32, address: ValidAccountId, feed_address: ValidAccountId,
                          state: u8) {
        self.assert_owner();
        let asset = Asset {
            name,
            symbol,
            standard,
            decimals,
            address: address.as_ref().clone(),
            feed_address: feed_address.as_ref().clone(),
            collateral_ratio: 0,
            state,
        };
        self.raft_list.insert(address.as_ref(), &asset);
    }

    pub(crate) fn assert_owner(&self) {
        assert_eq!(env::predecessor_account_id(), self.owner_id, "{}", errors::Unauthorized);
    }
}