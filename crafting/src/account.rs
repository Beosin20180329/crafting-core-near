use near_contract_standards::fungible_token::core_impl::ext_fungible_token;

use near_sdk::{
    assert_one_yocto, env, near_bindgen,
    AccountId, Balance, PromiseResult, StorageUsage,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;

use crate::utils::{ext_self, NO_DEPOSIT, ONE_YOCTO, GAS_FOR_FT_TRANSFER, GAS_FOR_RESOLVE_TRANSFER};
use crate::*;

const U32_STORAGE: StorageUsage = 4;
const U64_STORAGE: StorageUsage = 8;
const U128_STORAGE: StorageUsage = 16;

/// max length of account id is 64 bytes. We charge per byte.
const ACC_ID_STORAGE: StorageUsage = 64;
/// As a key, 4 bytes length would be added to the head
const ACC_ID_AS_KEY_STORAGE: StorageUsage = ACC_ID_STORAGE + 4;
const KEY_PREFIX_ACC: StorageUsage = 64;
/// As a near_sdk::collection key, 1 byte for prefiex
const ACC_ID_AS_CLT_KEY_STORAGE: StorageUsage = ACC_ID_AS_KEY_STORAGE + 1;

// ACC_ID: the Contract accounts map key length
// + VAccount enum: 1 byte
// + U128_STORAGE: near_amount storage
// + U32_STORAGE: tokens HashMap length
// + U64_STORAGE: storage_used
pub const INIT_ACCOUNT_STORAGE: StorageUsage =
    ACC_ID_AS_CLT_KEY_STORAGE + 1 + U128_STORAGE + U32_STORAGE + U64_STORAGE;

#[derive(BorshDeserialize, BorshSerialize)]
pub enum VAccount {
    Current(Account),
}

impl VAccount {
    pub fn into_current(self, _account_id: &AccountId) -> Account {
        match self {
            VAccount::Current(account) => account,
        }
    }
}

impl From<Account> for VAccount {
    fn from(account: Account) -> Self {
        VAccount::Current(account)
    }
}

/// Account deposits information and storage cost.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    /// Native NEAR amount sent to the exchange.
    pub near_amount: Balance,
    /// Amounts of various tokens deposited to this account.
    pub tokens: UnorderedMap<AccountId, Balance>,
    pub storage_used: StorageUsage,
}

impl Account {
    pub fn new(account_id: &AccountId) -> Self {
        Account {
            near_amount: 0,
            tokens: UnorderedMap::new(StorageKey::AccountTokens {
                account_id: account_id.clone(),
            }),
            storage_used: 0,
        }
    }

    pub fn get_balance(&self, token_id: &AccountId) -> Option<Balance> {
        if let Some(token_balance) = self.tokens.get(token_id) {
            Some(token_balance)
        } else {
            None
        }
    }

    pub fn get_tokens(&self) -> Vec<AccountId> {
        self.tokens.keys().collect()
    }

    /// Deposit amount to the balance of given token.
    /// if given token not register and not enough storage, deposit fails
    pub(crate) fn deposit_with_storage_check(&mut self, token_id: &AccountId, amount: Balance) -> bool {
        if let Some(balance) = self.tokens.get(token_id) {
            // token has been registered, just add without storage check
            let new_balance = balance + amount;
            self.tokens.insert(token_id, &new_balance);
            true
        } else {
            // check storage after insert, if fail should unregister the token
            self.tokens.insert(token_id, &(amount));
            if self.storage_usage() <= self.near_amount {
                true
            } else {
                self.tokens.remove(token_id);
                false
            }
        }
    }

    /// Deposit amount to the balance of given token.
    pub(crate) fn deposit(&mut self, token_id: &AccountId, amount: Balance) {
        if let Some(x) = self.tokens.get(token_id) {
            self.tokens.insert(token_id, &(amount + x));
        } else {
            self.tokens.insert(token_id, &amount);
        }
    }

    /// Withdraw amount of `token` from the internal balance.
    /// Panics if `amount` is bigger than the current balance.
    pub(crate) fn withdraw(&mut self, token_id: &AccountId, amount: Balance) {
        if let Some(x) = self.tokens.get(token_id) {
            assert!(x >= amount, "{}", errors::NOT_ENOUGH_TOKENS);
            self.tokens.insert(token_id, &(x - amount));
        } else {
            env::panic_str(errors::TOKEN_NOT_REG);
        }
    }

    /// Returns amount of $NEAR necessary to cover storage used by this data structure.
    pub fn storage_usage(&self) -> Balance {
        (INIT_ACCOUNT_STORAGE +
            self.tokens.len() as u64 * (KEY_PREFIX_ACC + ACC_ID_AS_KEY_STORAGE + U128_STORAGE)
        ) as u128 * env::storage_byte_cost()
    }

    /// Returns how much NEAR is available for storage.
    pub fn storage_available(&self) -> Balance {
        let locked = self.storage_usage();
        if self.near_amount > locked {
            self.near_amount - locked
        } else {
            0
        }
    }

    /// Asserts there is sufficient amount of $NEAR to cover storage usage.
    pub fn assert_storage_usage(&self) {
        assert!(
            self.storage_usage() <= self.near_amount,
            "{}",
            errors::INSUFFICIENT_STORAGE);
    }

    /// Returns minimal account deposit storage usage possible.
    pub fn min_storage_usage() -> Balance {
        INIT_ACCOUNT_STORAGE as Balance * env::storage_byte_cost()
    }

    /// Registers given token and set balance to 0.
    pub(crate) fn register(&mut self, token_ids: &Vec<AccountId>) {
        for token_id in token_ids {
            if self.get_balance(token_id).is_none() {
                self.tokens.insert(token_id, &0);
            }
        }
    }

    /// Unregisters `token_id` from this account balance.
    /// Panics if the `token_id` balance is not 0.
    pub(crate) fn unregister(&mut self, token_id: &AccountId) {
        let amount = self.tokens.remove(token_id).unwrap_or_default();
        assert_eq!(amount, 0, "{}", errors::NON_ZERO_TOKEN_BALANCE);
    }
}

#[near_bindgen]
impl Contract {

    /// Registers given token in the user's account deposit.
    /// Fails if not enough balance on this account to cover storage.
    #[payable]
    pub fn register_tokens(&mut self, token_ids: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);
        account.register(&token_ids);
        self.internal_save_account(&sender_id, account);
    }

    /// Unregister given token from user's account deposit.
    /// Panics if the balance of any given token is non 0.
    #[payable]
    pub fn unregister_tokens(&mut self, token_ids: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);
        for token_id in token_ids {
            account.unregister(&token_id);
        }
        self.internal_save_account(&sender_id, account);
    }

    #[private]
    pub fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128
    ) {
        assert_eq!(
            env::promise_results_count(),
            1,
            "{}",
            errors::CALLBACK_POST_WITHDRAW_INVALID
        );

        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(_) => {}
            PromiseResult::Failed => {
                // This reverts the changes from withdraw function.
                // If account doesn't exit, deposits to the owner's account as lostfound.
                let mut failed = false;
                if let Some(mut account) = self.internal_get_account(&sender_id) {
                    if account.deposit_with_storage_check(&token_id, amount.0) {
                        // cause storage already checked, here can directly save
                        self.accounts.insert(&sender_id, &account.into());
                    } else {
                        env::log_str(format!(
                            "Account {} has not enough storage. Depositing to owner.",
                            sender_id
                        ).as_str(),
                        );
                        failed = true;
                    }
                } else {
                    env::log_str(format!(
                        "Account {} is not registered. Depositing to owner.",
                        sender_id
                    ).as_str(),
                    );
                    failed = true;
                }
                if failed {
                    self.internal_lostfound(&token_id, amount.0);
                }
            }
        };
    }
}

impl Contract {

    /// Checks that account has enough storage to be stored and saves it into collection.
    /// This should be only place to directly use `self.accounts`.
    pub(crate) fn internal_save_account(&mut self, account_id: &AccountId, account: Account) {
        account.assert_storage_usage();
        self.accounts.insert(&account_id, &account.into());
    }

    /// save token to owner account as lostfound, no need to care about storage
    /// only global whitelisted token can be stored in lost-found
    pub(crate) fn internal_lostfound(&mut self, token_id: &AccountId, amount: u128) {
        if self.whitelisted_tokens.contains(token_id) {
            let mut lostfound = self.internal_unwrap_or_default_account(&self.owner_id);
            lostfound.deposit(token_id, amount);
            self.accounts.insert(&self.owner_id, &lostfound.into());
        } else {
            env::panic_str("ERR: non-whitelisted token can NOT deposit into lost-found.");
        }
    }

    /// Registers account in deposited amounts with given amount of $NEAR.
    /// If account already exists, adds amount to it.
    /// This should be used when it's known that storage is prepaid.
    pub(crate) fn internal_register_account(&mut self, account_id: &AccountId, amount: Balance) {
        let mut account = self.internal_unwrap_or_default_account(&account_id);
        account.near_amount += amount;
        self.internal_save_account(&account_id, account);
    }

    /// storage withdraw
    pub(crate) fn internal_storage_withdraw(&mut self, account_id: &AccountId, amount: Balance) -> u128 {
        let mut account = self.internal_unwrap_account(&account_id);
        let available = account.storage_available();
        assert!(available > 0, "ERR_NO_STORAGE_CAN_WITHDRAW");
        let mut withdraw_amount = amount;
        if amount == 0 {
            withdraw_amount = available;
        }
        assert!(withdraw_amount <= available, "ERR_STORAGE_WITHDRAW_TOO_MUCH");
        account.near_amount -= withdraw_amount;
        self.internal_save_account(&account_id, account);
        withdraw_amount
    }

    /// Record deposit of some number of tokens to this contract.
    /// Fails if account is not registered or if token isn't whitelisted.
    pub(crate) fn internal_deposit(
        &mut self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
    ) {
        let mut account = self.internal_unwrap_account(sender_id);
        assert!(
            self.whitelisted_tokens.contains(token_id)
                || account.get_balance(token_id).is_some(),
            "{}",
            errors::TOKEN_NOT_WHITELISTED
        );
        account.deposit(token_id, amount);
        self.internal_save_account(&sender_id, account);
    }

    pub fn internal_get_account(&self, account_id: &AccountId) -> Option<Account> {
        self.accounts
            .get(account_id)
            .map(|va| va.into_current(account_id))
    }

    pub fn internal_unwrap_account(&self, account_id: &AccountId) -> Account {
        self.internal_get_account(account_id)
            .expect(errors::ACC_NOT_REGISTERED)
    }

    pub fn internal_unwrap_or_default_account(&self, account_id: &AccountId) -> Account {
        self.internal_get_account(account_id)
            .unwrap_or_else(|| Account::new(account_id))
    }

    /// Returns current balance of given token for given user. If there is nothing recorded, returns 0.
    pub(crate) fn internal_get_deposit(
        &self,
        sender_id: &AccountId,
        token_id: &AccountId,
    ) -> Balance {
        self.internal_get_account(sender_id)
            .and_then(|x| x.get_balance(token_id))
            .unwrap_or(0)
    }

    /// Sends given amount to given user and if it fails, returns it back to user's balance.
    /// Tokens must already be subtracted from internal balance.
    pub(crate) fn internal_send_tokens(
        &self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
    ) -> Promise {
        ext_fungible_token::ft_transfer(
            sender_id.clone(),
            U128(amount),
            None,
            token_id.clone(),
            ONE_YOCTO,
            GAS_FOR_FT_TRANSFER,
        ).then(ext_self::exchange_callback_post_withdraw(
            token_id.clone(),
            sender_id.clone(),
            U128(amount),
            env::current_account_id(),
            NO_DEPOSIT,
            GAS_FOR_RESOLVE_TRANSFER,
        ))
    }
}
