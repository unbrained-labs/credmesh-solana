use anchor_lang::prelude::*;

#[error_code]
pub enum AttestorRegistryError {
    #[msg("Governance signature required (Squads CPI verification failed)")]
    GovernanceRequired,
    #[msg("Invalid attestor kind byte — see crates/credmesh-shared::AttestorKind")]
    InvalidKind,
}
