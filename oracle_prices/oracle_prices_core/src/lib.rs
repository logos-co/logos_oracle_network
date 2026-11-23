//use serde::{Deserialize, Serialize};

/*
/// Example state struct — customize for your program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramState {
    pub initialized: bool,
    pub owner: [u8; 32],
}
*/

use spel_framework::prelude::*;
// use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum AttestationStatus {
    #[default]
    Pending,   // written by the proposer, dispute window open
    Final,     // window passed with no successful dispute, or the winning set confirmed the written value
    Corrected, // replaced by the median of the set that won a dispute
    Voided,    // the proposer's committed set did not hold N valid observations; the round carries no price
}

#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct AttestedPrice {
    pub feed_id: [u8; 32],         // asset pair identifier, e.g. hash("BTC/USDT")
    pub price: u64,                // attested median, real value = price * 10^(-decimals), replaced on CORRECTED
    pub decimals: u32,             // number of decimal places in `price`
    pub round: u64,                // round identifier, in Bedrock block-height terms
    pub confidence: u64,           // dispersion of the N observations, scaled like `price`
    pub status: AttestationStatus, // set only by the contract; a new write is always PENDING
    pub set_hash: [u8; 32],        // from the submission
    pub proposer_id: [u8; 32],     // oracle_id of the writer, set by the contract
    pub written_at: u64,           // LEZ block that included the submission, set by the contract
}
/// Number of LEZ blocks an attestation stays PENDING (dispute window) before it can be finalized
/// Note: should be shorter than a round (see oracle_node indexer)
pub const DISPUTE_WINDOW_BLOCKS: u64 = 2;

/// State of a price feed account
#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct FeedState {
    pub pending: Option<AttestedPrice>,    // last submitted attestation, dispute window open
    pub last_final: Option<AttestedPrice>, // last finalized attestation (use this one to read a price)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedStateError {
    RoundAlreadyFinalized { round: u64, last_final_round: u64 },
    RoundAlreadyPending { round: u64, pending_round: u64 },
    DisputeWindowOpen { pending_round: u64, closes_at: u64, now: u64 },
    NoPendingAttestation { round: u64 },
}

impl FeedStateError {
    pub fn code(&self) -> u32 {
        match self {
            FeedStateError::RoundAlreadyFinalized { .. } => 1,
            FeedStateError::RoundAlreadyPending { .. } => 2,
            FeedStateError::DisputeWindowOpen { .. } => 3,
            FeedStateError::NoPendingAttestation { .. } => 4,
        }
    }
}

impl std::fmt::Display for FeedStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedStateError::RoundAlreadyFinalized { round, last_final_round } => write!(
                f, "Round {round} already finalized (last final round: {last_final_round})"
            ),
            FeedStateError::RoundAlreadyPending { round, pending_round } => write!(
                f, "Round {round} is not newer than pending round {pending_round}"
            ),
            FeedStateError::DisputeWindowOpen { pending_round, closes_at, now } => write!(
                f, "Dispute window for round {pending_round} still open (closes at block {closes_at}, now: {now})"
            ),
            FeedStateError::NoPendingAttestation { round } => write!(
                f, "No pending attestation for round {round}"
            ),
        }
    }
}

impl FeedState {
    /// Store a new attestation as PENDING (written at block `now`)
    ///
    /// If a previous attestation is pending and its dispute window is over, it is finalized first
    pub fn submit(&mut self, mut attestation: AttestedPrice, now: u64) -> Result<(), FeedStateError> {
        if let Some(last_final) = &self.last_final {
            if attestation.round <= last_final.round {
                return Err(FeedStateError::RoundAlreadyFinalized {
                    round: attestation.round,
                    last_final_round: last_final.round,
                });
            }
        }

        if let Some(pending) = &self.pending {
            if attestation.round <= pending.round {
                return Err(FeedStateError::RoundAlreadyPending {
                    round: attestation.round,
                    pending_round: pending.round,
                });
            }
            Self::check_window_closed(pending, now)?;
            self.promote_pending();
        }

        attestation.status = AttestationStatus::Pending;
        attestation.written_at = now;
        self.pending = Some(attestation);
        Ok(())
    }

    /// Finalize the pending attestation for `round` (dispute window must be over at block `now`)
    pub fn finalize(&mut self, round: u64, now: u64) -> Result<(), FeedStateError> {
        match &self.pending {
            Some(pending) if pending.round == round => {
                Self::check_window_closed(pending, now)?;
                self.promote_pending();
                Ok(())
            }
            _ => Err(FeedStateError::NoPendingAttestation { round }),
        }
    }

    fn check_window_closed(pending: &AttestedPrice, now: u64) -> Result<(), FeedStateError> {
        let closes_at = pending.written_at.saturating_add(DISPUTE_WINDOW_BLOCKS);
        if now < closes_at {
            return Err(FeedStateError::DisputeWindowOpen {
                pending_round: pending.round,
                closes_at,
                now,
            });
        }
        Ok(())
    }

    fn promote_pending(&mut self) {
        if let Some(mut pending) = self.pending.take() {
            pending.status = AttestationStatus::Final;
            self.last_final = Some(pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attestation(round: u64, price: u64) -> AttestedPrice {
        AttestedPrice { round, price, ..Default::default() }
    }

    #[test]
    fn test_submit_is_pending() {
        let mut state = FeedState::default();
        state.submit(attestation(1, 100), 10).unwrap();

        let pending = state.pending.as_ref().unwrap();
        assert_eq!(pending.status, AttestationStatus::Pending);
        assert_eq!(pending.written_at, 10);
        assert_eq!(pending.price, 100);
        assert!(state.last_final.is_none());
    }

    #[test]
    fn test_finalize() {
        let mut state = FeedState::default();
        state.submit(attestation(1, 100), 10).unwrap();

        assert_eq!(
            state.finalize(1, 10 + DISPUTE_WINDOW_BLOCKS - 1),
            Err(FeedStateError::DisputeWindowOpen {
                pending_round: 1,
                closes_at: 10 + DISPUTE_WINDOW_BLOCKS,
                now: 10 + DISPUTE_WINDOW_BLOCKS - 1,
            })
        );
        assert_eq!(state.finalize(2, 10 + DISPUTE_WINDOW_BLOCKS), Err(FeedStateError::NoPendingAttestation { round: 2 }));

        state.finalize(1, 10 + DISPUTE_WINDOW_BLOCKS).unwrap();
        assert!(state.pending.is_none());
        let last_final = state.last_final.as_ref().unwrap();
        assert_eq!(last_final.status, AttestationStatus::Final);
        assert_eq!(last_final.round, 1);
        assert_eq!(last_final.written_at, 10);

        // Already finalized
        assert_eq!(state.finalize(1, 100), Err(FeedStateError::NoPendingAttestation { round: 1 }));
    }

    #[test]
    fn test_submit_finalizes_previous_pending() {
        let mut state = FeedState::default();
        state.submit(attestation(1, 100), 10).unwrap();

        // Previous pending still in its dispute window
        assert!(matches!(
            state.submit(attestation(2, 200), 10 + DISPUTE_WINDOW_BLOCKS - 1),
            Err(FeedStateError::DisputeWindowOpen { .. })
        ));
        assert_eq!(state.pending.as_ref().unwrap().round, 1);

        state.submit(attestation(2, 200), 10 + DISPUTE_WINDOW_BLOCKS).unwrap();
        assert_eq!(state.last_final.as_ref().unwrap().round, 1);
        assert_eq!(state.last_final.as_ref().unwrap().status, AttestationStatus::Final);
        assert_eq!(state.pending.as_ref().unwrap().round, 2);
        assert_eq!(state.pending.as_ref().unwrap().status, AttestationStatus::Pending);
    }

    #[test]
    fn test_submit_rejects_old_rounds() {
        let mut state = FeedState::default();
        state.submit(attestation(5, 100), 10).unwrap();

        assert_eq!(
            state.submit(attestation(5, 101), 100),
            Err(FeedStateError::RoundAlreadyPending { round: 5, pending_round: 5 })
        );
        assert!(matches!(state.submit(attestation(4, 101), 100), Err(FeedStateError::RoundAlreadyPending { .. })));

        state.finalize(5, 100).unwrap();
        assert_eq!(
            state.submit(attestation(5, 101), 100),
            Err(FeedStateError::RoundAlreadyFinalized { round: 5, last_final_round: 5 })
        );
        state.submit(attestation(6, 101), 100).unwrap();
    }
}
