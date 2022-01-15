/// some error code may be unused (future use)
pub const Unauthorized: &str = "You are not admin";
pub const NoPermission: &str = "You do not have permission";
pub const NotSyntheticUsd: &str = "Not synthetic USD asset";
pub const OutdatedOracle: &str = "Oracle price is outdated";
pub const WithdrawLimit: &str = "Withdraw limit";
pub const CollateralAccountError: &str = "Invalid collateral account";
pub const InvalidAssetsList: &str = "Invalid Assets List";
pub const InvalidLiquidation: &str = "Invalid Liquidation";
pub const InvalidSigner: &str = "Invalid signer";
pub const ExchangeLiquidationAccount: &str = "Invalid exchange liquidation account";
pub const LiquidationDeadline: &str = "Liquidation deadline not passed";
pub const NoRewards: &str = "No rewards to claim";
pub const FundAccountError: &str = "Invalid fund account";
pub const SwapUnavailable: &str = "Swap Unavailable";
pub const Uninitialized: &str = "Assets list is not initialized";
pub const NoAssetFound: &str = "No asset with such address was found";
pub const MaxSupply: &str = "Asset max_supply crossed";
pub const NotCollateral: &str = "Asset is not collateral";
pub const InsufficientValueTrade: &str = "Insufficient value trade";
pub const InsufficientAmountAdminWithdraw: &str = "Insufficient amount admin withdraw";
pub const SettlementNotReached: &str = "Settlement slot not reached";
pub const UsdSettlement: &str = "Cannot settle rUSD";
pub const ParameterOutOfRange: &str = "Parameter out of range";
pub const Overflow: &str = "Overflow";
pub const DifferentScale: &str = "Scale is different";
pub const MismatchedTokens: &str = "Tokens does not represent same asset";
pub const SwaplineLimit: &str = "Limit crossed";
pub const CollateralLimitExceeded: &str = "Limit of collateral exceeded";
pub const UserBorrowLimit: &str = "User borrow limit";
pub const VaultBorrowLimit: &str = "Vault borrow limit";
pub const VaultWithdrawLimit: &str = "Vault withdraw limit";
pub const InvalidAccount: &str = "Invalid Account";
pub const PriceConfidenceOutOfRange: &str = "Price confidence out of range";
pub const InvalidOracleProgram: &str = "Invalid oracle program";
pub const InvalidExchangeAccount: &str = "Invalid exchange account";
pub const NoAttachedDeposit: &str = "Requires positive attached deposit";
pub const SyntheticAmountError: &str = "Invalid synthetic amount";
pub const CONTRACT_PAUSED: &str = "Contract paused";
pub const ILLEGAL_FEE: &str = "Illegal fee";
pub const TOKEN_NOT_REG: &str = "Token not registered";
pub const NOT_ENOUGH_TOKENS: &str = "Not enough tokens in deposit";
pub const ACC_NOT_REGISTERED: &str = "Account not registered";
pub const INSUFFICIENT_STORAGE: &str = "Insufficient $NEAR storage deposit";
pub const TOKEN_NOT_WHITELISTED: &str = "Token not whitelisted";
pub const CALLBACK_POST_WITHDRAW_INVALID: &str = "Expected 1 promise result from withdraw";
pub const ILLEGAL_WITHDRAW_AMOUNT: &str = "Illegal withdraw amount";
pub const NON_ZERO_TOKEN_BALANCE: &str = "Non-zero token balance";
