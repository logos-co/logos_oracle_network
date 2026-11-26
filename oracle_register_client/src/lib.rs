mod oracle_register_client;

pub use oracle_register_client::{
    compute_register_pda, OracleRegisterClient, OracleRegisterInstruction, RegisterAccounts,
    InitializeAccounts};
pub use oracle_register_core::RegisterState;
