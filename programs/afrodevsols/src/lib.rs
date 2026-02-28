// programs/afrodevsols/src/lib.rs

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod state;
pub mod instructions;

use instructions::*;

declare_id!("5UHiP59UBysX4yhJ3pdsdVK2QV6wtjAfB6RsZqztWZiL");

// ── EVENTS ───────────────────────────────────────────────────
// Declared at crate root so Anchor 0.32.1 can resolve them.
// instructions.rs imports these via `use crate::EventName;`

#[event]
pub struct ClaimEvent {
    pub claimer: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
    pub claimer_total: u64,
    pub claimer_claim_count: u64,
    pub cooldown_ends_at: i64,
    pub tier_index: u8,
    pub was_referral: bool,
    pub referral_bonus_applied: u64,
}

#[event]
pub struct ReferralConfirmedEvent {
    pub referrer: Pubkey,
    pub referred: Pubkey,
    pub timestamp: i64,
    pub bonus_queued_for_referrer: u64,
    pub bonus_applied_to_referred: u64,
}

#[event]
pub struct ReferralBonusClaimedEvent {
    pub referrer: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct SpecialGrantEvent {
    pub recipient: Pubkey,
    pub amount: u64,
    pub reason: String,
    pub is_public: bool,
    pub authority: Pubkey,
    pub timestamp: i64,
    pub new_treasury_balance: u64,
}

#[event]
pub struct BulkGrantEvent {
    pub recipients: Vec<Pubkey>,
    pub amounts: Vec<u64>,
    pub reason: String,
    pub is_public: bool,
    pub batch_id: i64,
    pub total_amount: u64,
    pub authority: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ConfigUpdatedEvent {
    pub authority: Pubkey,
    pub timestamp: i64,
    pub field_changed: String,
}

#[event]
pub struct TreasuryFundedEvent {
    pub funder: Pubkey,
    pub amount: u64,
    pub new_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct WithdrawalEvent {
    pub authority: Pubkey,
    pub amount: u64,
    pub destination: Pubkey,
    pub new_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct WalletBlockedEvent {
    pub target_wallet: Pubkey,
    pub is_blocked: bool,
    pub authority: Pubkey,
    pub timestamp: i64,
}

// ── PROGRAM ──────────────────────────────────────────────────

#[program]
pub mod afrodevsols {
    use super::*;

    /// One-time setup. Creates FaucetConfig and TreasuryVault.
    pub fn initialize(
        ctx: Context<Initialize>,
        min_amount: u64,
        max_amount: u64,
        cooldown_tier_amounts: [u64; 4],
        cooldown_tier_seconds: [i64; 4],
        daily_global_limit: u64,
        referral_bonus_claimer: u64,
        referral_bonus_referrer: u64,
    ) -> Result<()> {
        handle_initialize(
            ctx,
            min_amount,
            max_amount,
            cooldown_tier_amounts,
            cooldown_tier_seconds,
            daily_global_limit,
            referral_bonus_claimer,
            referral_bonus_referrer,
        )
    }

    /// Anyone can fund the treasury.
    pub fn fund_treasury(ctx: Context<FundTreasury>, amount: u64) -> Result<()> {
        handle_fund_treasury(ctx, amount)
    }

    /// Core claim instruction with full validation.
    pub fn claim(
        ctx: Context<Claim>,
        amount: u64,
        referrer: Option<Pubkey>,
    ) -> Result<()> {
        handle_claim(ctx, amount, referrer)
    }

    /// Referrer collects accumulated referral bonuses.
    pub fn claim_referral_bonus(ctx: Context<ClaimReferralBonus>) -> Result<()> {
        handle_claim_referral_bonus(ctx)
    }

    /// Admin sends any amount to one wallet. Bypasses all rules.
    pub fn special_grant(
        ctx: Context<SpecialGrant>,
        recipient: Pubkey,
        amount: u64,
        reason: String,
        is_public: bool,
    ) -> Result<()> {
        handle_special_grant(ctx, recipient, amount, reason, is_public)
    }

    /// Admin updates any config field. All fields optional.
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        is_paused: Option<bool>,
        min_amount: Option<u64>,
        max_amount: Option<u64>,
        cooldown_tier_amounts: Option<[u64; 4]>,
        cooldown_tier_seconds: Option<[i64; 4]>,
        daily_global_limit: Option<u64>,
        referral_enabled: Option<bool>,
        referral_bonus_claimer: Option<u64>,
        referral_bonus_referrer: Option<u64>,
        new_authority: Option<Pubkey>,
    ) -> Result<()> {
        handle_update_config(
            ctx,
            is_paused,
            min_amount,
            max_amount,
            cooldown_tier_amounts,
            cooldown_tier_seconds,
            daily_global_limit,
            referral_enabled,
            referral_bonus_claimer,
            referral_bonus_referrer,
            new_authority,
        )
    }

    /// Admin bans or unbans a wallet.
    pub fn block_wallet(
        ctx: Context<BlockWallet>,
        target_wallet: Pubkey,
        block: bool,
    ) -> Result<()> {
        handle_block_wallet(ctx, target_wallet, block)
    }

    /// Admin emergency fund recovery.
    pub fn withdraw_treasury(
        ctx: Context<WithdrawTreasury>,
        amount: u64,
    ) -> Result<()> {
        handle_withdraw_treasury(ctx, amount)
    }

    /// Cleanup claimer record. Admin or user themselves.
    pub fn close_claimer_record(
        ctx: Context<CloseClaimerRecord>,
        target_wallet: Pubkey,
    ) -> Result<()> {
        handle_close_claimer_record(ctx, target_wallet)
    }
}