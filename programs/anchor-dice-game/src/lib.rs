use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

pub use instructions::*;

declare_id!("5fxgaiepamCwwwj8xdPkTQ2jUoCFwKdTrPPQxsCYtQaF");

#[program]
pub mod anchor_dice_game {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, amount: u64) -> Result<()> {
        instructions::initialize::handler(ctx, amount)
    }

    pub fn place_bet(
        ctx: Context<PlaceBet>,
        seed: u128,
        amount: u64,
        roll: u8,
        house_commitment: [u8; 32],
    ) -> Result<()> {
        instructions::place_bet::handler(ctx, seed, amount, roll, house_commitment)
    }

    pub fn refund_bet(ctx: Context<RefundBet>) -> Result<()> {
        instructions::refund_bet::handler(ctx)
    }

    pub fn commit_randomness(
        ctx: Context<CommitRandomness>,
        proof: RandomnessProof,
    ) -> Result<()> {
        instructions::commit_randomness::handler(ctx, proof)
    }

    pub fn resolve_bet(ctx: Context<ResolveBet>) -> Result<()> {
        instructions::resolve_bet::handler(ctx)
    }
}