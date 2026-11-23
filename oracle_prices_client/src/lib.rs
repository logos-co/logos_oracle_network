// mod my_counter_client;
mod oracle_prices_client;

pub use oracle_prices_client::{
    compute_feed_price_pda, compute_oracle_prices_account_pda, FinalizeAttestationAccounts,
    InitializeAccounts, InitializeFeedAccounts, OraclePricesClient, OraclePricesInstruction,
    SubmitAttestationAccounts,
};
