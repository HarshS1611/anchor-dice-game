use anchor_lang::prelude::*;

use crate::{
    errors::DiceError,
    instructions::place_bet::REFUND_SLOT_WINDOW,
    state::{Bet, House},
};

#[derive(Accounts)]
pub struct RefundBet<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        seeds = [b"house", house.authority.as_ref()],
        bump = house.bump,
    )]
    pub house: Account<'info, House>,

    #[account(
        mut,
        close = player,
        seeds = [b"bet", player.key().as_ref(), bet.seed.to_le_bytes().as_ref()],
        bump = bet.bump,
        has_one = player @ DiceError::UnauthorizedRefund,
    )]
    pub bet: Account<'info, Bet>,

    pub system_program: Program<'info, System>,
}

pub(crate) fn handler(ctx: Context<RefundBet>) -> Result<()> {
    let bet = &ctx.accounts.bet;
    let current_slot = Clock::get()?.slot;

    require!(
        current_slot >= bet.slot + REFUND_SLOT_WINDOW,
        DiceError::RefundTooEarly
    );

    msg!(
        "Refund issued — player: {}, amount: {}, slots waited: {}",
        bet.player,
        bet.amount,
        current_slot - bet.slot,
    );
    Ok(())
}
