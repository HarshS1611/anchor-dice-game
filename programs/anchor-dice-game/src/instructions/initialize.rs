use anchor_lang::prelude::*;

use crate::state::House;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + House::INIT_SPACE,
        seeds = [b"house", authority.key().as_ref()],
        bump,
    )]
    pub house: Account<'info, House>,

    #[account(
        mut,
        seeds = [b"vault", house.key().as_ref()],
        bump,
    )]
    /// CHECK: SOL vault PDA, no data
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub(crate) fn handler(ctx: Context<Initialize>, amount: u64) -> Result<()> {
    let house = &mut ctx.accounts.house;
    house.authority = ctx.accounts.authority.key();
    house.bump = ctx.bumps.house;
    house.vault_bump = ctx.bumps.vault;

    if amount > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                },
            ),
            amount,
        )?;
    }

    msg!("House initialised. Authority: {}", house.authority);
    Ok(())
}
