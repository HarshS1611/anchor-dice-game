use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked, ID as IX_SYSVAR_ID,
};
use solana_program::hash::hashv;

use crate::{
    errors::DiceError,
    instructions::commit_randomness::RandomnessProof,
    state::{Bet, House},
};

pub const COMMIT_RANDOMNESS_DISCRIMINATOR: [u8; 8] = [146, 52, 195, 220, 79, 30, 53, 26];

#[derive(Accounts)]
pub struct ResolveBet<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"house", authority.key().as_ref()],
        bump = house.bump,
        has_one = authority @ DiceError::UnauthorizedHouse,
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
        mut,
        close = authority,
        seeds = [b"bet", bet.player.as_ref(), bet.seed.to_le_bytes().as_ref()],
        bump = bet.bump,
    )]
    pub bet: Account<'info, Bet>,

    /// CHECK: verified via bet.player address constraint
    #[account(mut, address = bet.player)]
    pub player: SystemAccount<'info>,

    /// CHECK: verified by address constraint
    #[account(address = IX_SYSVAR_ID)]
    pub instruction_sysvar: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub(crate) fn handler(ctx: Context<ResolveBet>) -> Result<()> {
    let current_index = load_current_index_checked(&ctx.accounts.instruction_sysvar)
        .map_err(|_| DiceError::MissingCommitInstruction)?;
    require!(current_index >= 1, DiceError::MissingCommitInstruction);

    let ix = load_instruction_at_checked(0, &ctx.accounts.instruction_sysvar)
        .map_err(|_| DiceError::MissingCommitInstruction)?;

    require_keys_eq!(ix.program_id, crate::ID, DiceError::WrongCommitProgram);

    require!(
        ix.data.len() >= 8 && ix.data[..8] == COMMIT_RANDOMNESS_DISCRIMINATOR,
        DiceError::WrongCommitDiscriminator
    );

    let proof = RandomnessProof::try_from_slice(&ix.data[8..])
        .map_err(|_| DiceError::DeserializeProofFailed)?;

    require_keys_eq!(proof.bet, ctx.accounts.bet.key(), DiceError::BetMismatch);

    let entropy_hash = hashv(&[&proof.entropy]);
    require!(
        entropy_hash.to_bytes() == ctx.accounts.bet.house_commitment,
        DiceError::CommitmentMismatch
    );

    require!(
        !ix.accounts.is_empty()
            && ix.accounts[0].pubkey == ctx.accounts.authority.key()
            && ix.accounts[0].is_signer,
        DiceError::WrongHouseAuthority
    );

    let bet = &ctx.accounts.bet;
    let player_seed_bytes: [u8; 16] = bet.seed.to_le_bytes();

    let mixed = hashv(&[
        player_seed_bytes.as_ref(),
        proof.entropy.as_ref(),
        bet.key().as_ref(),
    ]);
    let random_byte = mixed.to_bytes()[0];
    let random_roll = ((random_byte as u64 * 100) / 255 + 1) as u8;

    msg!(
        "ResolveBet — random_byte: {}, random_roll: {}, bet.roll: {}",
        random_byte,
        random_roll,
        bet.roll
    );

    if random_roll < bet.roll {
        let payout = bet
            .amount
            .checked_mul(100)
            .ok_or(DiceError::PayoutOverflow)?
            .checked_div(bet.roll as u64)
            .ok_or(DiceError::PayoutOverflow)?;

        msg!("Player wins! Payout: {} lamports", payout);

        let house_key = ctx.accounts.house.key();
        let vault_seeds: &[&[u8]] = &[
            b"vault",
            house_key.as_ref(),
            &[ctx.accounts.house.vault_bump],
        ];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.player.to_account_info(),
                },
                &[vault_seeds],
            ),
            payout,
        )?;
    } else {
        msg!(
            "House wins! random_roll ({}) >= bet.roll ({})",
            random_roll,
            bet.roll
        );
    }

    Ok(())
}
