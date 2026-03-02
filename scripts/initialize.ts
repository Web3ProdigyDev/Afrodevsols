// scripts/initialize.ts
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { Afrodevsols } from "../target/types/afrodevsols";

async function main() {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Afrodevsols as Program<Afrodevsols>;

    const [faucetConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("faucet_config")],
        program.programId
    );
    const [treasuryVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("treasury_vault")],
        program.programId
    );

    console.log("Program ID:    ", program.programId.toString());
    console.log("FaucetConfig:  ", faucetConfigPDA.toString());
    console.log("TreasuryVault: ", treasuryVaultPDA.toString());
    console.log("Authority:     ", provider.wallet.publicKey.toString());
    console.log("");

    // ── STEP 1: INITIALIZE ───────────────────────────────────

    const TIER_AMOUNTS = [
        new anchor.BN(0.1 * LAMPORTS_PER_SOL),
        new anchor.BN(0.25 * LAMPORTS_PER_SOL),
        new anchor.BN(0.5 * LAMPORTS_PER_SOL),
        new anchor.BN(1.0 * LAMPORTS_PER_SOL),
    ];

    const TIER_SECONDS = [
        new anchor.BN(6 * 3600),  // 6h
        new anchor.BN(12 * 3600),  // 12h
        new anchor.BN(24 * 3600),  // 24h
        new anchor.BN(48 * 3600),  // 48h
    ];

    const tx = await program.methods
        .initialize(
            new anchor.BN(0.1 * LAMPORTS_PER_SOL),   // min_amount
            new anchor.BN(1.0 * LAMPORTS_PER_SOL),   // max_amount
            TIER_AMOUNTS,
            TIER_SECONDS,
            new anchor.BN(50 * LAMPORTS_PER_SOL),    // daily_global_limit
            new anchor.BN(0.05 * LAMPORTS_PER_SOL),  // referral_bonus_claimer
            new anchor.BN(0.1 * LAMPORTS_PER_SOL),  // referral_bonus_referrer
        )
        .accounts({
            authority: provider.wallet.publicKey,
        } as any)
        .rpc();

    console.log("✅ Initialize tx:  ", tx);
    console.log("   https://explorer.solana.com/tx/" + tx + "?cluster=devnet");
    console.log("");

    // ── STEP 2: FUND TREASURY ────────────────────────────────

    const fundTx = await program.methods
        .fundTreasury(new anchor.BN(1 * LAMPORTS_PER_SOL))
        .accounts({
            funder: provider.wallet.publicKey,
        } as any)
        .rpc();

    console.log("✅ Fund treasury tx:", fundTx);
    console.log("   https://explorer.solana.com/tx/" + fundTx + "?cluster=devnet");
    console.log("");

    // ── VERIFY ───────────────────────────────────────────────

    const config = await program.account.faucetConfig.fetch(faucetConfigPDA);
    const balance = await provider.connection.getBalance(treasuryVaultPDA);

    console.log("── On-chain state ──────────────────────────");
    console.log("Authority:      ", config.authority.toString());
    console.log("Is paused:      ", config.isPaused);
    console.log("Min amount:     ", config.minAmount.toNumber() / LAMPORTS_PER_SOL, "SOL");
    console.log("Max amount:     ", config.maxAmount.toNumber() / LAMPORTS_PER_SOL, "SOL");
    console.log("Referral:       ", config.referralEnabled);
    console.log("Treasury bal:   ", balance / LAMPORTS_PER_SOL, "SOL");
    console.log("────────────────────────────────────────────");
    console.log("");
    console.log("🌍 Afrodevsols is LIVE on devnet!");
    console.log("🔍 https://explorer.solana.com/address/" + program.programId + "?cluster=devnet");
}

main().catch(console.error);