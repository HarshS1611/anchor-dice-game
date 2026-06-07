use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct House {
    pub authority: Pubkey,
    pub bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Bet {
    pub player: Pubkey,
    pub seed: u128,
    pub slot: u64,
    pub amount: u64,
    pub roll: u8,
    pub house_commitment: [u8; 32],
    pub bump: u8,
}
