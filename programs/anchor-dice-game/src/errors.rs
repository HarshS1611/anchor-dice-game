use anchor_lang::prelude::*;

#[error_code]
pub enum DiceError {
    #[msg("Roll must be between 2 and 96 (inclusive)")]
    InvalidRoll,

    #[msg("Bet amount must be greater than zero")]
    ZeroBetAmount,

    #[msg("House vault does not have enough liquidity to cover potential payout")]
    InsufficientHouseLiquidity,

    #[msg("Bet cannot be refunded yet; not enough slots have passed")]
    RefundTooEarly,

    #[msg("Only the original bettor can request a refund")]
    UnauthorizedRefund,

    #[msg("Expected commit_randomness instruction at index 0 of this transaction")]
    MissingCommitInstruction,

    #[msg("commit_randomness must target this program")]
    WrongCommitProgram,

    #[msg("commit_randomness data did not match the expected discriminator")]
    WrongCommitDiscriminator,

    #[msg("Failed to deserialize the RandomnessProof from commit_randomness data")]
    DeserializeProofFailed,

    #[msg("RandomnessProof.bet field does not match the Bet account being resolved")]
    BetMismatch,

    #[msg("house_commitment stored in Bet does not match hash(entropy) in the proof")]
    CommitmentMismatch,

    #[msg("House authority in commit_randomness accounts does not match stored authority")]
    WrongHouseAuthority,

    #[msg("Overflow when computing payout amount")]
    PayoutOverflow,

    #[msg("Only the house authority can perform this action")]
    UnauthorizedHouse,

    #[msg("Cannot withdraw more than the house vault balance")]
    WithdrawExceedsBalance,
}
