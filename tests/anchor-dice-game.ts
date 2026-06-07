import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";

import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import { createHash } from "crypto";
import { assert } from "chai";
import { AnchorDiceGame } from "../target/types/anchor_dice_game";

function sha256(data: Buffer): Buffer {
  return createHash("sha256").update(data).digest();
}

function u128ToLeBytes(n: bigint): Buffer {
  const buf = Buffer.alloc(16);
  buf.writeBigUInt64LE(n & BigInt("0xffffffffffffffff"), 0);
  buf.writeBigUInt64LE(n >> BigInt(64), 8);
  return buf;
}

function findHousePda(authority: PublicKey, programId: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("house"), authority.toBuffer()],
    programId
  );
}

function findVaultPda(house: PublicKey, programId: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), house.toBuffer()],
    programId
  );
}

function findBetPda(player: PublicKey, seed: bigint, programId: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("bet"), player.toBuffer(), u128ToLeBytes(seed)],
    programId
  );
}

function makeEntropy(): { entropy: Buffer; commitment: number[] } {
  const entropy = Buffer.from(
    Array.from({ length: 32 }, () => Math.floor(Math.random() * 256))
  );
  return { entropy, commitment: Array.from(sha256(entropy)) };
}

const SOL = 1_000_000_000n;
const INIT_VAULT = 2n * SOL;
const BET_AMOUNT = 100_000_000n;
const ROLL = 50;

describe("anchor-dice-game", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.AnchorDiceGame as Program<AnchorDiceGame>;
  const programId = program.programId;

  const houseKp = (provider.wallet as anchor.Wallet).payer;
  const playerKp = Keypair.generate();

  let housePda: PublicKey;
  let vaultPda: PublicKey;

  const betSeed = BigInt(Date.now());
  let betPda: PublicKey;
  let entropy: Buffer;
  let commitment: number[];

  before(async () => {
    [housePda] = findHousePda(houseKp.publicKey, programId);
    [vaultPda] = findVaultPda(housePda, programId);
    [betPda] = findBetPda(playerKp.publicKey, betSeed, programId);

    const sig = await provider.connection.requestAirdrop(
      playerKp.publicKey,
      Number(2n * SOL)
    );
    await provider.connection.confirmTransaction(sig, "confirmed");
  });

  it("initialize: creates house PDA and funds vault", async () => {
    await program.methods
      .initialize(new BN(INIT_VAULT.toString()))
      .accountsPartial({
        authority: houseKp.publicKey,
        house: housePda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([houseKp])
      .rpc();

    const house = await program.account.house.fetch(housePda);
    assert.equal(house.authority.toBase58(), houseKp.publicKey.toBase58());

    const vaultBal = await provider.connection.getBalance(vaultPda);
    assert.isAtLeast(vaultBal, Number(INIT_VAULT));
    console.log("    vault funded:", vaultBal / Number(SOL), "SOL");
  });

  it("place_bet: creates Bet PDA and escrows player SOL", async () => {
    ({ entropy, commitment } = makeEntropy());

    await program.methods
      .placeBet(
        new BN(betSeed.toString()),
        new BN(BET_AMOUNT.toString()),
        ROLL,
        commitment
      )
      .accountsPartial({
        player: playerKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: betPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerKp])
      .rpc();

    const bet = await program.account.bet.fetch(betPda);
    assert.equal(bet.player.toBase58(), playerKp.publicKey.toBase58());
    assert.equal(bet.roll, ROLL);
    assert.equal(bet.amount.toString(), BET_AMOUNT.toString());
    console.log("    Bet PDA created, slot:", bet.slot.toString());
  });

  it("place_bet: rejects roll = 1 (below MIN_ROLL)", async () => {
    const seed2 = betSeed + 1n;
    const [badBetPda] = findBetPda(playerKp.publicKey, seed2, programId);
    const { commitment: c2 } = makeEntropy();

    try {
      await program.methods
        .placeBet(new BN(seed2.toString()), new BN(BET_AMOUNT.toString()), 1, c2)
        .accountsPartial({
          player: playerKp.publicKey,
          house: housePda,
          vault: vaultPda,
          bet: badBetPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([playerKp])
        .rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.message, "InvalidRoll");
      console.log("    ✓ InvalidRoll rejected");
    }
  });

  it("place_bet: rejects roll = 97 (above MAX_ROLL)", async () => {
    const seed3 = betSeed + 2n;
    const [badBetPda] = findBetPda(playerKp.publicKey, seed3, programId);
    const { commitment: c3 } = makeEntropy();

    try {
      await program.methods
        .placeBet(new BN(seed3.toString()), new BN(BET_AMOUNT.toString()), 97, c3)
        .accountsPartial({
          player: playerKp.publicKey,
          house: housePda,
          vault: vaultPda,
          bet: badBetPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([playerKp])
        .rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.message, "InvalidRoll");
      console.log("    ✓ InvalidRoll (97) rejected");
    }
  });

  it("resolve_bet: settles bet via instruction introspection", async () => {
    const playerBefore = await provider.connection.getBalance(playerKp.publicKey);
    const vaultBefore = await provider.connection.getBalance(vaultPda);

    const commitIx = await program.methods
      .commitRandomness({ bet: betPda, entropy: Array.from(entropy) })
      .accountsPartial({
        authority: houseKp.publicKey,
        house: housePda,
        bet: betPda,
      })
      .instruction();

    const resolveIx = await program.methods
      .resolveBet()
      .accountsPartial({
        authority: houseKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: betPda,
        player: playerKp.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const tx = new Transaction().add(commitIx, resolveIx);
    const sig = await provider.sendAndConfirm(tx, [houseKp]);
    console.log("    resolve tx:", sig);

    const betInfo = await provider.connection.getAccountInfo(betPda);
    assert.isNull(betInfo, "Bet PDA should be closed");

    const playerAfter = await provider.connection.getBalance(playerKp.publicKey);
    const vaultAfter = await provider.connection.getBalance(vaultPda);
    console.log(
      "    player delta:",
      (playerAfter - playerBefore) / Number(SOL),
      "SOL | vault delta:",
      (vaultAfter - vaultBefore) / Number(SOL),
      "SOL"
    );
  });

  it("resolve_bet: rejects wrong entropy (commitment mismatch)", async () => {
    const seed4 = betSeed + 10n;
    const [bet4] = findBetPda(playerKp.publicKey, seed4, programId);
    const { commitment: c4 } = makeEntropy();

    await program.methods
      .placeBet(new BN(seed4.toString()), new BN(BET_AMOUNT.toString()), ROLL, c4)
      .accountsPartial({
        player: playerKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: bet4,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerKp])
      .rpc();

    const wrongEntropy = Array.from(Buffer.alloc(32, 0));

    const commitIx = await program.methods
      .commitRandomness({ bet: bet4, entropy: wrongEntropy })
      .accountsPartial({ authority: houseKp.publicKey, house: housePda, bet: bet4 })
      .instruction();

    const resolveIx = await program.methods
      .resolveBet()
      .accountsPartial({
        authority: houseKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: bet4,
        player: playerKp.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    try {
      const tx = new Transaction().add(commitIx, resolveIx);
      await provider.sendAndConfirm(tx, [houseKp]);
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.message, "CommitmentMismatch");
      console.log("    ✓ CommitmentMismatch correctly caught");
    }
  });

  it("resolve_bet: fails when commit_randomness is not ix[0]", async () => {
    const seed5 = betSeed + 20n;
    const [bet5] = findBetPda(playerKp.publicKey, seed5, programId);
    const { commitment: c5 } = makeEntropy();

    await program.methods
      .placeBet(new BN(seed5.toString()), new BN(BET_AMOUNT.toString()), ROLL, c5)
      .accountsPartial({
        player: playerKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: bet5,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerKp])
      .rpc();

    try {
      await program.methods
        .resolveBet()
        .accountsPartial({
          authority: houseKp.publicKey,
          house: housePda,
          vault: vaultPda,
          bet: bet5,
          player: playerKp.publicKey,
          instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
          systemProgram: SystemProgram.programId,
        })
        .signers([houseKp])
        .rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.message, "MissingCommitInstruction");
      console.log("    ✓ MissingCommitInstruction correctly caught");
    }
  });

  it("refund_bet: rejects refund before slot window", async () => {
    const seed6 = betSeed + 30n;
    const [bet6] = findBetPda(playerKp.publicKey, seed6, programId);
    const { commitment: c6 } = makeEntropy();

    await program.methods
      .placeBet(new BN(seed6.toString()), new BN(BET_AMOUNT.toString()), ROLL, c6)
      .accountsPartial({
        player: playerKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: bet6,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerKp])
      .rpc();

    try {
      await program.methods
        .refundBet()
        .accountsPartial({
          player: playerKp.publicKey,
          house: housePda,
          bet: bet6,
          systemProgram: SystemProgram.programId,
        })
        .signers([playerKp])
        .rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.message, "RefundTooEarly");
      console.log("    ✓ RefundTooEarly correctly enforced");
    }
  });

  it("refund_bet: rejects refund from non-player signer", async () => {
    const seed7 = betSeed + 40n;
    const [bet7] = findBetPda(playerKp.publicKey, seed7, programId);
    const { commitment: c7 } = makeEntropy();

    await program.methods
      .placeBet(new BN(seed7.toString()), new BN(BET_AMOUNT.toString()), ROLL, c7)
      .accountsPartial({
        player: playerKp.publicKey,
        house: housePda,
        vault: vaultPda,
        bet: bet7,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerKp])
      .rpc();

    const impostor = Keypair.generate();
    const drop = await provider.connection.requestAirdrop(
      impostor.publicKey,
      Number(SOL)
    );
    await provider.connection.confirmTransaction(drop, "confirmed");

    try {
      await program.methods
        .refundBet()
        .accountsPartial({
          player: impostor.publicKey,
          house: housePda,
          bet: bet7,
          systemProgram: SystemProgram.programId,
        })
        .signers([impostor])
        .rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      const msg: string = e.message;
      const caught =
        msg.includes("UnauthorizedRefund") ||
        msg.includes("ConstraintHasOne") ||
        msg.includes("has_one") ||
        msg.includes("AnchorError");
      assert.isTrue(caught, `unexpected error: ${msg}`);
      console.log("    ✓ Unauthorised refund rejected");
    }
  });
});
