# Anchor Dice Game

A Solana program built with Anchor that implements a provably fair dice game using **instruction introspection** for commit-reveal randomness.

## How It Works

Randomness on-chain is hard — the house controls when to resolve, so a naive design lets it cherry-pick outcomes. This program prevents that with a two-phase commit-reveal scheme enforced entirely on-chain via the Solana instructions sysvar.

### Commit-Reveal Flow

1. **Before accepting a bet**, the house generates 32 bytes of secret entropy and computes `sha256(entropy)`. This hash is the *commitment* — it locks the house into a specific entropy value before the outcome is known.

2. **Player places the bet** (`place_bet`), passing the commitment hash. The commitment is stored in the `Bet` PDA. Neither party can change it after this point.

3. **To resolve**, the house must send **two instructions in the same transaction**:
   - `commit_randomness` at index 0 — reveals the original entropy inside a `RandomnessProof` struct
   - `resolve_bet` at index 1 — uses the instructions sysvar to read back `commit_randomness` from the same transaction, verifies `sha256(entropy) == house_commitment`, then derives the outcome

Because both instructions are atomic (same transaction), the house cannot submit `commit_randomness` and observe the outcome before deciding whether to call `resolve_bet`. The result is determined and settled in one atomic step.

### Outcome Derivation

The winning roll is derived as:

```
mixed = sha256(player_seed_bytes || house_entropy || bet_pubkey)
random_roll = (mixed[0] * 100 / 255) + 1   // maps to 1..=100
player wins if random_roll < bet.roll
```

Both the player's seed (chosen at bet time) and the house entropy contribute to the outcome. Neither party can pre-determine the result without knowing the other's input.

### Instruction Introspection

`resolve_bet` reads `commit_randomness` directly from the transaction's instruction data via the Solana instructions sysvar (`load_instruction_at_checked`). It verifies:

- The instruction at index 0 targets this program
- Its 8-byte Anchor discriminator matches `commit_randomness`
- The `RandomnessProof` deserializes correctly and references the same `Bet` account
- `sha256(proof.entropy)` matches the `house_commitment` stored in the `Bet`
- The house authority signed `commit_randomness`

This is the core of Challenge 1 from the Week 6 Instruction Introspection assignment — using a **custom struct** (`RandomnessProof`) embedded in instruction data rather than an Ed25519 signature.

## Program Structure

```
programs/anchor-dice-game/src/
├── lib.rs                          # Program entry point and instruction dispatch
├── errors.rs                       # DiceError enum
├── state/
│   ├── mod.rs
│   └── state.rs                    # House and Bet account structs
└── instructions/
    ├── mod.rs
    ├── initialize.rs               # Create House PDA and fund vault
    ├── place_bet.rs                # Player places a bet
    ├── commit_randomness.rs        # House reveals entropy (must be ix[0])
    ├── resolve_bet.rs              # Settle bet via instruction introspection
    └── refund_bet.rs               # Player reclaims funds after timeout
```

## Accounts

### House PDA
Seeds: `["house", authority]`

| Field | Type | Description |
|---|---|---|
| `authority` | `Pubkey` | Wallet allowed to resolve bets |
| `bump` | `u8` | PDA bump |
| `vault_bump` | `u8` | Bump for the associated vault PDA |

### Vault PDA
Seeds: `["vault", house]` — plain system account holding house liquidity (no data).

### Bet PDA
Seeds: `["bet", player, seed.to_le_bytes()]`

| Field | Type | Description |
|---|---|---|
| `player` | `Pubkey` | Player who placed the bet |
| `seed` | `u128` | Player-chosen random seed; part of PDA derivation |
| `slot` | `u64` | Slot at placement; used for refund timeout |
| `amount` | `u64` | Lamports wagered |
| `roll` | `u8` | Player wins if resolved roll < this (range: 2–96) |
| `house_commitment` | `[u8; 32]` | sha256 of the house entropy, committed at bet time |
| `bump` | `u8` | PDA bump |

## Instructions

### `initialize(amount: u64)`
Creates the `House` PDA and transfers `amount` lamports from the authority into the vault.

### `place_bet(seed: u128, amount: u64, roll: u8, house_commitment: [u8; 32])`
Creates a `Bet` PDA and escrows `amount` lamports from the player. Valid rolls: 2–96. Requires vault to hold enough liquidity to cover the maximum possible payout (`amount * 100 / roll`).

### `commit_randomness(proof: RandomnessProof)`
Signed by the house authority. Embeds the `RandomnessProof` in transaction data — this instruction is intentionally minimal on-chain; its purpose is to be read back by `resolve_bet` via introspection.

```rust
pub struct RandomnessProof {
    pub bet: Pubkey,       // Bet account this proof is for
    pub entropy: [u8; 32], // Raw entropy whose sha256 equals Bet.house_commitment
}
```

### `resolve_bet()`
Must be called as the second instruction in a transaction where `commit_randomness` is the first. Reads `commit_randomness` from the instructions sysvar, verifies the entropy commitment, derives the outcome, pays out to the player or closes the bet to the house, and closes the `Bet` account.

**Payout (player wins):** `amount * 100 / roll` lamports from the vault  
**No payout (house wins):** bet escrow lamports go to the house authority via account close

### `refund_bet()`
Allows the player to reclaim their bet if the house fails to resolve within 1,000 slots. Enforces `has_one = player` so only the original bettor can refund.

## Running Tests

```bash
anchor test
```

All 9 tests must pass:

```
anchor-dice-game
  ✔ initialize: creates house PDA and funds vault
  ✔ place_bet: creates Bet PDA and escrows player SOL
  ✔ place_bet: rejects roll = 1 (below MIN_ROLL)
  ✔ place_bet: rejects roll = 97 (above MAX_ROLL)
  ✔ resolve_bet: settles bet via instruction introspection
  ✔ resolve_bet: rejects wrong entropy (commitment mismatch)
  ✔ resolve_bet: fails when commit_randomness is not ix[0]
  ✔ refund_bet: rejects refund before slot window
  ✔ refund_bet: rejects refund from non-player signer

9 passing
```

## Prerequisites

- [Rust](https://rustup.rs/)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor CLI](https://www.anchor-lang.com/docs/installation) v0.32.1
- [Node.js](https://nodejs.org/) + Yarn
