// tests/afrodevsols.ts

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, LAMPORTS_PER_SOL, Keypair } from "@solana/web3.js";
import { assert } from "chai";
import { Afrodevsols } from "../target/types/afrodevsols";

// ── HELPERS ──────────────────────────────────────────────────

function sol(amount: number): anchor.BN {
  return new anchor.BN(amount * LAMPORTS_PER_SOL);
}

function hours(n: number): anchor.BN {
  return new anchor.BN(n * 3600);
}

async function airdrop(
  connection: anchor.web3.Connection,
  wallet: PublicKey,
  amount = 2
) {
  const sig = await connection.requestAirdrop(wallet, amount * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(sig, "confirmed");
}

function getPDA(seeds: Buffer[], programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

// ── SETUP ────────────────────────────────────────────────────

describe("afrodevsols", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Afrodevsols as Program<Afrodevsols>;
  const authority = (provider.wallet as anchor.Wallet).payer;

  // PDAs
  const faucetConfigPDA = getPDA([Buffer.from("faucet_config")], program.programId);
  const treasuryVaultPDA = getPDA([Buffer.from("treasury_vault")], program.programId);

  // Test wallets
  const user1 = Keypair.generate();
  const user2 = Keypair.generate();
  const user3 = Keypair.generate(); // will be referrer

  // Standard config
  const TIER_AMOUNTS = [sol(0.1), sol(0.25), sol(0.5), sol(1.0)];
  const TIER_SECONDS = [hours(6), hours(12), hours(24), hours(48)];
  const DAILY_LIMIT = sol(50);
  const REFERRAL_BONUS_CLAIMER = sol(0.05);
  const REFERRAL_BONUS_REFERRER = sol(0.1);

  before(async () => {
    // Fund test wallets
    for (const wallet of [user1, user2, user3]) {
      await airdrop(provider.connection, wallet.publicKey);
      await new Promise((r) => setTimeout(r, 500));
    }
  });

  // ──────────────────────────────────────────────────────────
  // TEST 1: INITIALIZE
  // ──────────────────────────────────────────────────────────
  it("✅ initializes the faucet config", async () => {
    await program.methods
      .initialize(
        sol(0.1),
        sol(1.0),
        TIER_AMOUNTS,
        TIER_SECONDS,
        DAILY_LIMIT,
        REFERRAL_BONUS_CLAIMER,
        REFERRAL_BONUS_REFERRER
      )
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const config = await program.account.faucetConfig.fetch(faucetConfigPDA);

    assert.equal(config.authority.toString(), authority.publicKey.toString());
    assert.equal(config.isPaused, false);
    assert.equal(config.totalClaims.toString(), "0");
    assert.equal(config.totalUniqueClaimers.toString(), "0");
    assert.equal(config.referralEnabled, true);
    assert.equal(config.minAmount.toString(), sol(0.1).toString());
    assert.equal(config.maxAmount.toString(), sol(1.0).toString());

    console.log("    FaucetConfig PDA:", faucetConfigPDA.toString());
    console.log("    TreasuryVault PDA:", treasuryVaultPDA.toString());
  });

  // ──────────────────────────────────────────────────────────
  // TEST 2: FUND TREASURY
  // ──────────────────────────────────────────────────────────
  it("✅ funds the treasury", async () => {
    await program.methods
      .fundTreasury(sol(10))
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        funder: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const balance = await provider.connection.getBalance(treasuryVaultPDA);
    assert.isAtLeast(balance, 10 * LAMPORTS_PER_SOL);
    console.log("    Treasury balance:", balance / LAMPORTS_PER_SOL, "SOL");
  });

  // ──────────────────────────────────────────────────────────
  // TEST 3: VALID CLAIM — TIER 0 (0.1 SOL)
  // ──────────────────────────────────────────────────────────
  it("✅ allows a valid tier 0 claim (0.1 SOL)", async () => {
    const claimerRecordPDA = getPDA(
      [Buffer.from("claimer"), user1.publicKey.toBuffer()],
      program.programId
    );

    const balanceBefore = await provider.connection.getBalance(user1.publicKey);

    await program.methods
      .claim(sol(0.1), null)
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        claimerRecord: claimerRecordPDA,
        claimer: user1.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user1])
      .rpc();

    const balanceAfter = await provider.connection.getBalance(user1.publicKey);
    const record = await program.account.claimerRecord.fetch(claimerRecordPDA);
    const config = await program.account.faucetConfig.fetch(faucetConfigPDA);

    assert.isAbove(balanceAfter, balanceBefore, "Balance should increase");
    assert.equal(record.totalClaims.toString(), "1");
    assert.equal(record.wallet.toString(), user1.publicKey.toString());
    assert.equal(config.totalClaims.toString(), "1");
    assert.equal(config.totalUniqueClaimers.toString(), "1");

    console.log("    SOL received:", (balanceAfter - balanceBefore) / LAMPORTS_PER_SOL);
  });

  // ──────────────────────────────────────────────────────────
  // TEST 4: COOLDOWN ENFORCED — same tier, same user
  // ──────────────────────────────────────────────────────────
  it("✅ rejects claim on active cooldown", async () => {
    const claimerRecordPDA = getPDA(
      [Buffer.from("claimer"), user1.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .claim(sol(0.1), null)
        .accounts({
          faucetConfig: faucetConfigPDA,
          treasuryVault: treasuryVaultPDA,
          claimerRecord: claimerRecordPDA,
          claimer: user1.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      assert.fail("Should have thrown CooldownActive");
    } catch (e: any) {
      assert.include(e.message, "CooldownActive");
      console.log("    Cooldown correctly enforced ✓");
    }
  });

  // ──────────────────────────────────────────────────────────
  // TEST 5: DIFFERENT TIER — cooldowns are independent
  // ──────────────────────────────────────────────────────────
  it("✅ allows claim on different tier while tier 0 is on cooldown", async () => {
    const claimerRecordPDA = getPDA(
      [Buffer.from("claimer"), user1.publicKey.toBuffer()],
      program.programId
    );

    const balanceBefore = await provider.connection.getBalance(user1.publicKey);

    await program.methods
      .claim(sol(0.25), null)  // tier 1 — different cooldown
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        claimerRecord: claimerRecordPDA,
        claimer: user1.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user1])
      .rpc();

    const balanceAfter = await provider.connection.getBalance(user1.publicKey);
    const record = await program.account.claimerRecord.fetch(claimerRecordPDA);

    assert.isAbove(balanceAfter, balanceBefore);
    assert.equal(record.totalClaims.toString(), "2");
    console.log("    Independent tiers work ✓");
  });

  // ──────────────────────────────────────────────────────────
  // TEST 6: REFERRAL CLAIM — new user with referrer
  // ──────────────────────────────────────────────────────────
  it("✅ applies referral bonus on first claim for new user", async () => {
    // user3 is the referrer — needs a claimer record first
    const user3RecordPDA = getPDA(
      [Buffer.from("claimer"), user3.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .claim(sol(0.1), null)
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        claimerRecord: user3RecordPDA,
        claimer: user3.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user3])
      .rpc();

    // user2 claims with user3 as referrer
    const user2RecordPDA = getPDA(
      [Buffer.from("claimer"), user2.publicKey.toBuffer()],
      program.programId
    );

    const balanceBefore = await provider.connection.getBalance(user2.publicKey);

    await program.methods
      .claim(sol(0.1), user3.publicKey)  // referrer = user3
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        claimerRecord: user2RecordPDA,
        claimer: user2.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user2])
      .rpc();

    const balanceAfter = await provider.connection.getBalance(user2.publicKey);
    const record = await program.account.claimerRecord.fetch(user2RecordPDA);

    // user2 should receive 0.1 SOL + 0.05 SOL referral bonus = 0.15 SOL
    const received = balanceAfter - balanceBefore;
    assert.isAbove(received, 0.14 * LAMPORTS_PER_SOL, "Should include referral bonus");
    assert.equal(record.referredBy?.toString(), user3.publicKey.toString());

    console.log("    user2 received:", received / LAMPORTS_PER_SOL, "SOL (with referral bonus)");
  });

  // ──────────────────────────────────────────────────────────
  // TEST 7: SELF REFERRAL — should be ignored silently
  // ──────────────────────────────────────────────────────────
  it("✅ silently ignores self-referral", async () => {
    const newUser = Keypair.generate();
    await airdrop(provider.connection, newUser.publicKey);

    const recordPDA = getPDA(
      [Buffer.from("claimer"), newUser.publicKey.toBuffer()],
      program.programId
    );

    const balanceBefore = await provider.connection.getBalance(newUser.publicKey);

    await program.methods
      .claim(sol(0.1), newUser.publicKey)  // referring yourself
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        claimerRecord: recordPDA,
        claimer: newUser.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([newUser])
      .rpc();

    const balanceAfter = await provider.connection.getBalance(newUser.publicKey);
    const record = await program.account.claimerRecord.fetch(recordPDA);

    // Should receive exactly 0.1 SOL, no bonus
    const received = balanceAfter - balanceBefore;
    assert.isBelow(received, 0.11 * LAMPORTS_PER_SOL, "Should not include self-referral bonus");
    assert.isNull(record.referredBy, "referredBy should be null");

    console.log("    Self-referral ignored ✓, received:", received / LAMPORTS_PER_SOL);
  });

  // ──────────────────────────────────────────────────────────
  // TEST 8: PAUSE — blocks all claims
  // ──────────────────────────────────────────────────────────
  it("✅ pause blocks claims, unpause restores them", async () => {
    // Pause
    await program.methods
      .updateConfig(true, null, null, null, null, null, null, null, null, null)
      .accounts({
        faucetConfig: faucetConfigPDA,
        authority: authority.publicKey,
      })
      .rpc();

    const freshUser = Keypair.generate();
    await airdrop(provider.connection, freshUser.publicKey);
    const recordPDA = getPDA(
      [Buffer.from("claimer"), freshUser.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .claim(sol(0.1), null)
        .accounts({
          faucetConfig: faucetConfigPDA,
          treasuryVault: treasuryVaultPDA,
          claimerRecord: recordPDA,
          claimer: freshUser.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([freshUser])
        .rpc();
      assert.fail("Should have thrown FaucetPaused");
    } catch (e: any) {
      assert.include(e.message, "FaucetPaused");
    }

    // Unpause
    await program.methods
      .updateConfig(false, null, null, null, null, null, null, null, null, null)
      .accounts({
        faucetConfig: faucetConfigPDA,
        authority: authority.publicKey,
      })
      .rpc();

    const config = await program.account.faucetConfig.fetch(faucetConfigPDA);
    assert.equal(config.isPaused, false);
    console.log("    Pause/unpause works ✓");
  });

  // ──────────────────────────────────────────────────────────
  // TEST 9: BLOCK WALLET
  // ──────────────────────────────────────────────────────────
  it("✅ blocks a wallet from claiming", async () => {
    const user3RecordPDA = getPDA(
      [Buffer.from("claimer"), user3.publicKey.toBuffer()],
      program.programId
    );

    // Block user3
    await program.methods
      .blockWallet(user3.publicKey, true)
      .accounts({
        faucetConfig: faucetConfigPDA,
        claimerRecord: user3RecordPDA,
        authority: authority.publicKey,
      })
      .rpc();

    const record = await program.account.claimerRecord.fetch(user3RecordPDA);
    assert.equal(record.isBlocked, true);

    // Unblock for cleanup
    await program.methods
      .blockWallet(user3.publicKey, false)
      .accounts({
        faucetConfig: faucetConfigPDA,
        claimerRecord: user3RecordPDA,
        authority: authority.publicKey,
      })
      .rpc();

    const recordAfter = await program.account.claimerRecord.fetch(user3RecordPDA);
    assert.equal(recordAfter.isBlocked, false);
    console.log("    Block/unblock works ✓");
  });

  // ──────────────────────────────────────────────────────────
  // TEST 10: SPECIAL GRANT — bypasses all rules
  // ──────────────────────────────────────────────────────────
  it("✅ admin can special grant any amount bypassing all rules", async () => {
    const config = await program.account.faucetConfig.fetch(faucetConfigPDA);
    const nonce = config.totalClaims;

    const grantRecordPDA = getPDA(
      [
        Buffer.from("grant_record"),
        authority.publicKey.toBuffer(),
        user2.publicKey.toBuffer(),
        nonce.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    const balanceBefore = await provider.connection.getBalance(user2.publicKey);

    // 5 SOL — way above the 1 SOL max, proving rules are bypassed
    await program.methods
      .specialGrant(user2.publicKey, sol(5), "hackathon-prize", true)
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        recipientWallet: user2.publicKey,
        grantRecord: grantRecordPDA,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const balanceAfter = await provider.connection.getBalance(user2.publicKey);
    const grant = await program.account.grantRecord.fetch(grantRecordPDA);

    assert.isAbove(balanceAfter, balanceBefore + 4.9 * LAMPORTS_PER_SOL);
    assert.equal(grant.amount.toString(), sol(5).toString());
    assert.equal(grant.isPublic, true);

    console.log("    Special grant sent:", sol(5).toNumber() / LAMPORTS_PER_SOL, "SOL ✓");
  });

  // ──────────────────────────────────────────────────────────
  // TEST 11: WITHDRAW TREASURY
  // ──────────────────────────────────────────────────────────
  it("✅ admin can withdraw from treasury", async () => {
    const balanceBefore = await provider.connection.getBalance(authority.publicKey);
    const treasuryBefore = await provider.connection.getBalance(treasuryVaultPDA);

    await program.methods
      .withdrawTreasury(sol(1))
      .accounts({
        faucetConfig: faucetConfigPDA,
        treasuryVault: treasuryVaultPDA,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const treasuryAfter = await provider.connection.getBalance(treasuryVaultPDA);
    assert.isBelow(treasuryAfter, treasuryBefore);
    console.log("    Treasury before:", treasuryBefore / LAMPORTS_PER_SOL);
    console.log("    Treasury after: ", treasuryAfter / LAMPORTS_PER_SOL);
  });

  // ──────────────────────────────────────────────────────────
  // TEST 12: UNAUTHORIZED — non-admin blocked
  // ──────────────────────────────────────────────────────────
  it("✅ rejects unauthorized admin calls", async () => {
    try {
      await program.methods
        .updateConfig(true, null, null, null, null, null, null, null, null, null)
        .accounts({
          faucetConfig: faucetConfigPDA,
          authority: user1.publicKey,  // NOT the real authority
        })
        .signers([user1])
        .rpc();
      assert.fail("Should have thrown Unauthorized");
    } catch (e: any) {
      assert.include(e.message, "Unauthorized");
      console.log("    Unauthorized access blocked ✓");
    }
  });

  // ──────────────────────────────────────────────────────────
  // TEST 13: INVALID AMOUNT — not a valid tier
  // ──────────────────────────────────────────────────────────
  it("✅ rejects invalid claim amount", async () => {
    const recordPDA = getPDA(
      [Buffer.from("claimer"), user1.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .claim(sol(0.3), null)  // 0.3 is not a valid tier
        .accounts({
          faucetConfig: faucetConfigPDA,
          treasuryVault: treasuryVaultPDA,
          claimerRecord: recordPDA,
          claimer: user1.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user1])
        .rpc();
      assert.fail("Should have thrown InvalidAmount");
    } catch (e: any) {
      assert.include(e.message, "InvalidAmount");
      console.log("    Invalid amount rejected ✓");
    }
  });

  // ──────────────────────────────────────────────────────────
  // TEST 14: FINAL STATE CHECK
  // ──────────────────────────────────────────────────────────
  it("✅ final state is consistent", async () => {
    const config = await program.account.faucetConfig.fetch(faucetConfigPDA);

    assert.isAbove(config.totalClaims.toNumber(), 0);
    assert.isAbove(config.totalUniqueClaimers.toNumber(), 0);
    assert.isAbove(config.totalSolDistributed.toNumber(), 0);
    assert.equal(config.isPaused, false);

    console.log("\n    ── Final State ──────────────────────────");
    console.log("    Total claims:    ", config.totalClaims.toString());
    console.log("    Unique claimers: ", config.totalUniqueClaimers.toString());
    console.log("    SOL distributed: ", config.totalSolDistributed.toNumber() / LAMPORTS_PER_SOL, "SOL");
    console.log("    Is paused:       ", config.isPaused);
    console.log("    ─────────────────────────────────────────");
  });
});