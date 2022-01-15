use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::AccountId;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PriceInfo {
    /// Mapping from assets to price of assets.
    prices: LookupMap<AccountId, u128>,
}

impl PriceInfo {
    pub fn new() -> Self {
        Self {
            prices: LookupMap::new(b"r".to_vec()),
        }
    }

    /// Returns the price of assets.
    pub fn get_price(&self, asset: AccountId) -> u128 {
        let opt = self.prices.get(&asset);
        assert!(opt.is_some());
        opt.unwrap()
    }

    /// Feed the price of assets.
    pub fn feed_price(&mut self, asset: AccountId, price: u128) {
        self.prices.insert(&asset, &price);
    }
}