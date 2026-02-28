// programs/afrodevsols/src/constants.rs

pub const FAUCET_CONFIG_SEED: &[u8] = b"faucet_config";
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";
pub const CLAIMER_SEED: &[u8] = b"claimer";
pub const REFERRAL_SEED: &[u8] = b"referral";
pub const GRANT_RECORD_SEED: &[u8] = b"grant_record";
pub const DAILY_STATS_SEED: &[u8] = b"daily_stats";

// Minimum SOL to always keep in treasury for rent reserves
// 0.01 SOL in lamports
pub const RENT_RESERVE_LAMPORTS: u64 = 10_000_000;

// Maximum recipients in a single bulk_grant call
pub const MAX_BULK_RECIPIENTS: usize = 20;

// Maximum length of a reason string
pub const MAX_REASON_LENGTH: usize = 64;

// Maximum length of a display name (stored off-chain but validated here)
pub const MAX_NAME_LENGTH: usize = 30;

// Cooldown tier count
pub const COOLDOWN_TIER_COUNT: usize = 4;

// Seconds in common time units
pub const SECONDS_PER_HOUR: i64 = 3600;
pub const SECONDS_PER_DAY: i64 = 86400;