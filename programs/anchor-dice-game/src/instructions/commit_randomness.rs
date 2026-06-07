use anchor_lang::prelude::*;

use crate::state::{Bet, House};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct RandomnessProof {
    pub bet: Pubkey,
    pub entropy: [u8; 32],
}

#[derive(Accounts)]
pub struct CommitRandomness<'info> {
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"house", authority.key().as_ref()],
        bump = house.bump,
        has_one = authority @ crate::errors::DiceError::UnauthorizedHouse,
    )]
    pub house: Account<'info, House>,

    pub bet: Account<'info, Bet>,
}

pub(crate) fn handler(ctx: Context<CommitRandomness>, proof: RandomnessProof) -> Result<()> {
    msg!(
        "CommitRandomness — bet: {}, entropy submitted by house authority: {}",
        proof.bet,
        ctx.accounts.authority.key()
    );
    Ok(())
}
