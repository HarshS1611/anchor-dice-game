use anchor_lang::prelude::*;

use crate::{
    errors::DiceError,
    state::{Bet, House},
};

pub const MAX_ROLL: u8 = 96;
pub const MIN_ROLL: u8 = 2;
pub const REFUND_SLOT_WINDOW: u64 = 1_000;

#[derive(Accounts)]
#[instruction(seed: u128)]
pub struct PlaceBet<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        seeds = [b"house", house.authority.as_ref()],
        bump = house.bump,
    )]
    pub house: Account<'info, House>,

    #[account(
        mut,
        seeds = [b"vault", house.key().as_ref()],
        bump = house.vault_bump,
    )]
    /// CHECK: SOL vault PDA, no data
    pub vault: SystemAccount<'info>,

    #[account(
        init,
        payer = player,
        space = 8 + Bet::INIT_SPACE,
        seeds = [b"bet", player.key().as_ref(), seed.to_le_bytes().as_ref()],
        bump,
    )]
    pub bet: Account<'info, Bet>,

    pub system_program: Program<'info, System>,
}

pub(crate) fn handler(
    ctx: Context<PlaceBet>,
    seed: u128,
    amount: u64,
    roll: u8,
    house_commitment: [u8; 32],
) -> Result<()> {
    require!(roll >= MIN_ROLL && roll <= MAX_ROLL, DiceError::InvalidRoll);
    require!(amount > 0, DiceError::ZeroBetAmount);

    let max_payout = amount
        .checked_mul(100)
        .ok_or(DiceError::PayoutOverflow)?
        .checked_div(roll as u64)
        .ok_or(DiceError::PayoutOverflow)?;

    let vault_balance = ctx.accounts.vault.lamports();
    require!(
        vault_balance >= max_payout,
        DiceError::InsufficientHouseLiquidity
    );

    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player.to_account_info(),
                to: ctx.accounts.bet.to_account_info(),
            },
        ),
        amount,
    )?;

    let bet = &mut ctx.accounts.bet;
    bet.player = ctx.accounts.player.key();
    bet.seed = seed;
    bet.slot = Clock::get()?.slot;
    bet.amount = amount;
    bet.roll = roll;
    bet.house_commitment = house_commitment;
    bet.bump = ctx.bumps.bet;

    msg!(
        "Bet placed — player: {}, amount: {}, roll: {}, slot: {}",
        bet.player,
        bet.amount,
        bet.roll,
        bet.slot
    );
    Ok(())
}
