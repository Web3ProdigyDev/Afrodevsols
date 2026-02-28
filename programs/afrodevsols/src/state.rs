// programs/afrodevsols/src/state.rs

use anchor_lang::prelude::*;
use crate::constants::COOLDOWN_TIER_COUNT;

// ============================================================
// FAUCET CONFIG
// One per program. Global settings and running totals.
// ============================================================
#[account]
pub struct FaucetConfig {
    pub authority: Pubkey,           // 32 — Admin wallet
    pub treasury: Pubkey,            // 32 — TreasuryVault PDA address
    pub is_paused: bool,             // 1  — Kill switch
    pub program_version: [u8; 3],   // 3  — [major, minor, patch]

    // Running totals
    pub total_sol_distributed: u64,  // 8  — Lifetime lamports out
    pub total_claims: u64,           // 8  — Lifetime claim count
    pub total_unique_claimers: u64,  // 8  — Distinct wallets served

    // Claim rules
    pub min_amount: u64,             // 8  — Floor (lamports)
    pub max_amount: u64,             // 8  — Ceiling for regular claims (lamports)

    // Cooldown tiers: [amount_lamports, cooldown_seconds] x4
    pub cooldown_tier_amounts: [u64; 4],   // 32
    pub cooldown_tier_seconds: [i64; 4],   // 32

    // Daily global cap
    pub daily_global_limit: u64,         // 8
    pub daily_global_distributed: u64,   // 8
    pub daily_reset_timestamp: i64,      // 8

    // Referral settings
    pub referral_enabled: bool,           // 1
    pub referral_bonus_claimer: u64,      // 8  — Bonus lamports for new user
    pub referral_bonus_referrer: u64,     // 8  — Bonus lamports queued for referrer

    pub bump: u8,                         // 1
}

impl FaucetConfig {
    // Space calculation: 8 (discriminator) + sum of all fields above
    pub const LEN: usize = 8 + 32 + 32 + 1 + 3 + 8 + 8 + 8 + 8 + 8 + 32 + 32 + 8 + 8 + 8 + 1 + 8 + 8 + 1;

    pub fn is_daily_reset_needed(&self, current_time: i64) -> bool {
        current_time >= self.daily_reset_timestamp + 86400
    }

    pub fn get_tier_index(&self, amount: u64) -> Option<usize> {
        for i in 0..COOLDOWN_TIER_COUNT {
            if self.cooldown_tier_amounts[i] == amount {
                return Some(i);
            }
        }
        None
    }
}

// ============================================================
// CLAIMER RECORD
// One per user wallet. Created on first claim.
// ============================================================
#[account]
pub struct ClaimerRecord {
    pub wallet: Pubkey,                          // 32
    pub total_claimed: u64,                      // 8  — Lifetime lamports received
    pub total_claims: u64,                       // 8  — How many times claimed
    pub last_claim_timestamp: i64,               // 8
    pub last_claim_amount: u64,                  // 8
    pub cooldown_ends_at: [i64; 4],             // 32 — One per tier, all independent
    pub is_blocked: bool,                        // 1
    pub referred_by: Option<Pubkey>,             // 33 (1 flag + 32 key)
    pub referral_count: u64,                     // 8  — How many they've referred
    pub pending_referral_bonus: u64,             // 8  — Uncollected referral rewards
    pub created_at: i64,                         // 8
    pub last_claim_slot: u64,                    // 8  — For double-spend prevention
    pub bump: u8,                                // 1
}

impl ClaimerRecord {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8 + 8 + 32 + 1 + 33 + 8 + 8 + 8 + 8 + 1;

    pub fn is_cooldown_active(&self, tier_index: usize, current_time: i64) -> bool {
        self.cooldown_ends_at[tier_index] > current_time
    }

    pub fn cooldown_remaining(&self, tier_index: usize, current_time: i64) -> i64 {
        (self.cooldown_ends_at[tier_index] - current_time).max(0)
    }
}

// ============================================================
// REFERRAL RECORD
// One per referral relationship.
// ============================================================
#[account]
pub struct ReferralRecord {
    pub referrer: Pubkey,               // 32
    pub referred: Pubkey,               // 32
    pub confirmed_at: i64,              // 8
    pub bonus_paid_to_referrer: bool,   // 1
    pub bonus_paid_to_referred: bool,   // 1
    pub bump: u8,                       // 1
}

impl ReferralRecord {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 1 + 1 + 1;
}

// ============================================================
// GRANT RECORD
// Created for every admin special_grant or bulk_grant.
// ============================================================
#[account]
pub struct GrantRecord {
    pub authority: Pubkey,    // 32
    pub recipient: Pubkey,    // 32
    pub amount: u64,          // 8
    pub reason: [u8; 64],    // 64 — Fixed size, padded with zeros
    pub timestamp: i64,       // 8
    pub grant_type: u8,       // 1  — 0 = special, 1 = bulk
    pub batch_id: i64,        // 8  — 0 for special grants, timestamp for bulk
    pub is_public: bool,      // 1
    pub bump: u8,             // 1
}

impl GrantRecord {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 64 + 8 + 1 + 8 + 1 + 1;
}

// ============================================================
// DAILY STATS
// One per calendar day. Created on first claim of each day.
// ============================================================
#[account]
pub struct DailyStats {
    pub date: i64,                    // 8  — Unix day number (timestamp / 86400)
    pub total_distributed: u64,       // 8
    pub total_claims: u64,            // 8
    pub unique_claimers: u64,         // 8
    pub largest_single_claim: u64,    // 8
    pub bump: u8,                     // 1
}

impl DailyStats {
    pub const LEN: usize = 8 + 8 + 8 + 8 + 8 + 8 + 1;

    pub fn day_number(timestamp: i64) -> i64 {
        timestamp / 86400
    }
}