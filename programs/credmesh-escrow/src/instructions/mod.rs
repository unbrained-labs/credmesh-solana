pub mod claim_and_settle;
pub mod deposit;
pub mod execute_params;
pub mod init_pool;
pub mod liquidate;
pub mod propose_params;
pub mod request_advance;
pub mod skim_protocol_fees;
pub mod withdraw;

// Wildcard re-exports are required by Anchor's `#[program]` macro: it
// generates code that looks up `crate::__client_accounts_*` and
// `crate::__cpi_client_accounts_*` symbols emitted by each
// `#[derive(Accounts)]`, plus the Accounts struct itself. Each module also
// has a `pub fn handler` — those nine names DO collide on a glob, but the
// collision is dormant because nothing references unqualified `handler`
// outside its own module (lib.rs uses the fully-qualified
// `instructions::<name>::handler` path, and downstream clients don't call
// handlers — they go through the program entry point).
pub use claim_and_settle::*;
pub use deposit::*;
pub use execute_params::*;
pub use init_pool::*;
pub use liquidate::*;
pub use propose_params::*;
pub use request_advance::*;
pub use skim_protocol_fees::*;
pub use withdraw::*;
