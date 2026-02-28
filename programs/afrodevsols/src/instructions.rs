// programs/afrodevsols/src/instructions.rs

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::*;
use crate::errors::AfrodevsError;
use crate::constants::*;
use crate::{
    ClaimEvent,
    ReferralConfirmedEvent,
    ReferralBonusClaimedEvent,
    SpecialGrantEvent,
    ConfigUpdatedEvent,
    TreasuryFundedEvent,
    WithdrawalEvent,
    WalletBlockedEvent,
};

// ============================================================
// INSTRUCTION 1: INITIALIZE
// Called once to set up the program.
// ============================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = FaucetConfig::LEN,
        seeds = [FAUCET_CONFIG_SEED],
        bump
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    /// CHECK: This is the treasury vault PDA — just holds SOL, no data
    #[account(
        mut,
        seeds = [TREASURY_VAULT_SEED],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(
    ctx: Context<Initialize>,
    min_amount: u64,
    max_amount: u64,
    cooldown_tier_amounts: [u64; 4],
    cooldown_tier_seconds: [i64; 4],
    daily_global_limit: u64,
    referral_bonus_claimer: u64,
    referral_bonus_referrer: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.faucet_config;
    let clock = Clock::get()?;

    config.authority = ctx.accounts.authority.key();
    config.treasury = ctx.accounts.treasury_vault.key();
    config.is_paused = false;
    config.program_version = [1, 0, 0];
    config.total_sol_distributed = 0;
    config.total_claims = 0;
    config.total_unique_claimers = 0;
    config.min_amount = min_amount;
    config.max_amount = max_amount;
    config.cooldown_tier_amounts = cooldown_tier_amounts;
    config.cooldown_tier_seconds = cooldown_tier_seconds;
    config.daily_global_limit = daily_global_limit;
    config.daily_global_distributed = 0;
    config.daily_reset_timestamp = clock.unix_timestamp;
    config.referral_enabled = true;
    config.referral_bonus_claimer = referral_bonus_claimer;
    config.referral_bonus_referrer = referral_bonus_referrer;
    config.bump = ctx.bumps.faucet_config;

    Ok(())
}

// ============================================================
// INSTRUCTION 2: FUND TREASURY
// Anyone can top up the treasury.
// ============================================================

#[derive(Accounts)]
pub struct FundTreasury<'info> {
    #[account(
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    /// CHECK: Treasury vault PDA — receives SOL
    #[account(
        mut,
        seeds = [TREASURY_VAULT_SEED],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    #[account(mut)]
    pub funder: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_fund_treasury(ctx: Context<FundTreasury>, amount: u64) -> Result<()> {
    require!(amount > 0, AfrodevsError::InvalidAmount);

    let cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.funder.to_account_info(),
            to: ctx.accounts.treasury_vault.to_account_info(),
        },
    );
    system_program::transfer(cpi_context, amount)?;

    let new_balance = ctx.accounts.treasury_vault.lamports();

    emit!(TreasuryFundedEvent {
        funder: ctx.accounts.funder.key(),
        amount,
        new_balance,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 3: CLAIM
// The main instruction. Full validation gauntlet.
// ============================================================

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(
        mut,
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    /// CHECK: Treasury vault PDA — sends SOL
    #[account(
        mut,
        seeds = [TREASURY_VAULT_SEED],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = claimer,
        space = ClaimerRecord::LEN,
        seeds = [CLAIMER_SEED, claimer.key().as_ref()],
        bump
    )]
    pub claimer_record: Account<'info, ClaimerRecord>,

    #[account(mut)]
    pub claimer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_claim(
    ctx: Context<Claim>,
    amount: u64,
    referrer: Option<Pubkey>,
) -> Result<()> {
    let config = &mut ctx.accounts.faucet_config;
    let claimer_record = &mut ctx.accounts.claimer_record;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    let current_slot = clock.slot;

    // ── VALIDATION GAUNTLET ──────────────────────────────────

    // 1. Program not paused
    require!(!config.is_paused, AfrodevsError::FaucetPaused);

    // 2. Amount matches a valid tier
    let tier_index = config.get_tier_index(amount)
        .ok_or(AfrodevsError::InvalidAmount)?;

    // 3. Amount within min/max
    require!(amount >= config.min_amount, AfrodevsError::AmountTooLow);
    require!(amount <= config.max_amount, AfrodevsError::AmountTooHigh);

    // 4. Reset daily counter if needed
    if config.is_daily_reset_needed(current_time) {
        config.daily_global_distributed = 0;
        config.daily_reset_timestamp = current_time;
    }

    // 5. Daily global limit not exceeded
    let projected_daily = config.daily_global_distributed
        .checked_add(amount)
        .ok_or(AfrodevsError::Overflow)?;
    require!(
        projected_daily <= config.daily_global_limit,
        AfrodevsError::DailyLimitReached
    );

    // 6. Treasury has enough (keeping rent reserve)
    let treasury_balance = ctx.accounts.treasury_vault.lamports();
    require!(
        treasury_balance >= amount + RENT_RESERVE_LAMPORTS,
        AfrodevsError::InsufficientTreasury
    );

    // 7. Wallet not blocked (only if record already existed)
    if claimer_record.total_claims > 0 {
        require!(!claimer_record.is_blocked, AfrodevsError::WalletBlocked);
    }

    // 8. Cooldown for this tier has expired
    if claimer_record.total_claims > 0 {
        require!(
            !claimer_record.is_cooldown_active(tier_index, current_time),
            AfrodevsError::CooldownActive
        );
    }

    // 9. Not same slot as last claim (double-spend prevention)
    require!(
        current_slot > claimer_record.last_claim_slot,
        AfrodevsError::CooldownActive
    );

    // ── REFERRAL HANDLING ────────────────────────────────────

    let mut referral_bonus_applied: u64 = 0;
    let mut was_referral = false;
    let is_new_claimer = claimer_record.total_claims == 0;

    if let Some(referrer_key) = referrer {
        if config.referral_enabled
            && is_new_claimer
            && referrer_key != ctx.accounts.claimer.key()
        {
            referral_bonus_applied = config.referral_bonus_claimer;
            was_referral = true;
            claimer_record.referred_by = Some(referrer_key);

            emit!(ReferralConfirmedEvent {
                referrer: referrer_key,
                referred: ctx.accounts.claimer.key(),
                timestamp: current_time,
                bonus_queued_for_referrer: config.referral_bonus_referrer,
                bonus_applied_to_referred: referral_bonus_applied,
            });
        }
    }

    let total_amount = amount
        .checked_add(referral_bonus_applied)
        .ok_or(AfrodevsError::Overflow)?;

    // Final treasury check with bonus included
    require!(
        treasury_balance >= total_amount + RENT_RESERVE_LAMPORTS,
        AfrodevsError::InsufficientTreasury
    );

    // ── EXECUTE TRANSFER ─────────────────────────────────────

    **ctx.accounts.treasury_vault.try_borrow_mut_lamports()? -= total_amount;
    **ctx.accounts.claimer.try_borrow_mut_lamports()? += total_amount;

    // ── UPDATE STATE ─────────────────────────────────────────

    let new_cooldown_end = current_time + config.cooldown_tier_seconds[tier_index];

    if is_new_claimer {
        claimer_record.wallet = ctx.accounts.claimer.key();
        claimer_record.created_at = current_time;
        claimer_record.cooldown_ends_at = [0i64; 4];
        claimer_record.bump = ctx.bumps.claimer_record;
        config.total_unique_claimers = config.total_unique_claimers
            .checked_add(1)
            .ok_or(AfrodevsError::Overflow)?;
    }

    claimer_record.total_claimed = claimer_record.total_claimed
        .checked_add(total_amount)
        .ok_or(AfrodevsError::Overflow)?;
    claimer_record.total_claims = claimer_record.total_claims
        .checked_add(1)
        .ok_or(AfrodevsError::Overflow)?;
    claimer_record.last_claim_timestamp = current_time;
    claimer_record.last_claim_amount = total_amount;
    claimer_record.cooldown_ends_at[tier_index] = new_cooldown_end;
    claimer_record.last_claim_slot = current_slot;

    config.total_sol_distributed = config.total_sol_distributed
        .checked_add(total_amount)
        .ok_or(AfrodevsError::Overflow)?;
    config.total_claims = config.total_claims
        .checked_add(1)
        .ok_or(AfrodevsError::Overflow)?;
    config.daily_global_distributed = projected_daily;

    // ── EMIT EVENT ───────────────────────────────────────────

    emit!(ClaimEvent {
        claimer: ctx.accounts.claimer.key(),
        amount: total_amount,
        timestamp: current_time,
        claimer_total: claimer_record.total_claimed,
        claimer_claim_count: claimer_record.total_claims,
        cooldown_ends_at: new_cooldown_end,
        tier_index: tier_index as u8,
        was_referral,
        referral_bonus_applied,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 4: CLAIM REFERRAL BONUS
// Referrer collects their accumulated rewards.
// ============================================================

#[derive(Accounts)]
pub struct ClaimReferralBonus<'info> {
    #[account(
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    /// CHECK: Treasury vault PDA
    #[account(
        mut,
        seeds = [TREASURY_VAULT_SEED],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [CLAIMER_SEED, referrer.key().as_ref()],
        bump = referrer_record.bump,
    )]
    pub referrer_record: Account<'info, ClaimerRecord>,

    #[account(mut)]
    pub referrer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_claim_referral_bonus(ctx: Context<ClaimReferralBonus>) -> Result<()> {
    let config = &ctx.accounts.faucet_config;
    let referrer_record = &mut ctx.accounts.referrer_record;
    let clock = Clock::get()?;

    require!(!config.is_paused, AfrodevsError::FaucetPaused);

    let bonus_amount = referrer_record.pending_referral_bonus;
    require!(bonus_amount > 0, AfrodevsError::NoPendingBonus);

    let treasury_balance = ctx.accounts.treasury_vault.lamports();
    require!(
        treasury_balance >= bonus_amount + RENT_RESERVE_LAMPORTS,
        AfrodevsError::InsufficientTreasury
    );

    **ctx.accounts.treasury_vault.try_borrow_mut_lamports()? -= bonus_amount;
    **ctx.accounts.referrer.try_borrow_mut_lamports()? += bonus_amount;

    referrer_record.pending_referral_bonus = 0;

    emit!(ReferralBonusClaimedEvent {
        referrer: ctx.accounts.referrer.key(),
        amount: bonus_amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 5: SPECIAL GRANT
// Admin sends any amount to one wallet. No rules apply.
// Uses total_claims as nonce so admin can grant same recipient multiple times.
// ============================================================

#[derive(Accounts)]
#[instruction(recipient: Pubkey, amount: u64, reason: String, is_public: bool)]
pub struct SpecialGrant<'info> {
    #[account(
        mut,
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
        has_one = authority @ AfrodevsError::Unauthorized,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    /// CHECK: Treasury vault PDA
    #[account(
        mut,
        seeds = [TREASURY_VAULT_SEED],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    /// CHECK: The recipient wallet — receives SOL
    #[account(mut)]
    pub recipient_wallet: AccountInfo<'info>,

    #[account(
        init,
        payer = authority,
        space = GrantRecord::LEN,
        seeds = [
            GRANT_RECORD_SEED,
            authority.key().as_ref(),
            recipient.as_ref(),
            &faucet_config.total_claims.to_le_bytes(),
        ],
        bump
    )]
    pub grant_record: Account<'info, GrantRecord>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_special_grant(
    ctx: Context<SpecialGrant>,
    recipient: Pubkey,
    amount: u64,
    reason: String,
    is_public: bool,
) -> Result<()> {
    require!(amount > 0, AfrodevsError::InvalidAmount);
    require!(!reason.is_empty(), AfrodevsError::EmptyReason);
    require!(reason.len() <= MAX_REASON_LENGTH, AfrodevsError::EmptyReason);

    let treasury_balance = ctx.accounts.treasury_vault.lamports();
    require!(
        treasury_balance >= amount + RENT_RESERVE_LAMPORTS,
        AfrodevsError::InsufficientTreasury
    );

    // Transfer
    **ctx.accounts.treasury_vault.try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.recipient_wallet.try_borrow_mut_lamports()? += amount;

    // Write grant record
    let grant = &mut ctx.accounts.grant_record;
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp;

    grant.authority = ctx.accounts.authority.key();
    grant.recipient = recipient;
    grant.amount = amount;
    grant.timestamp = timestamp;
    grant.grant_type = 0; // special
    grant.batch_id = 0;
    grant.is_public = is_public;
    grant.bump = ctx.bumps.grant_record;

    let mut reason_bytes = [0u8; 64];
    let reason_slice = reason.as_bytes();
    let copy_len = reason_slice.len().min(64);
    reason_bytes[..copy_len].copy_from_slice(&reason_slice[..copy_len]);
    grant.reason = reason_bytes;

    let new_treasury_balance = ctx.accounts.treasury_vault.lamports();

    emit!(SpecialGrantEvent {
        recipient,
        amount,
        reason,
        is_public,
        authority: ctx.accounts.authority.key(),
        timestamp,
        new_treasury_balance,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 6: UPDATE CONFIG
// Admin changes any setting.
// ============================================================

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
        has_one = authority @ AfrodevsError::Unauthorized,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    pub authority: Signer<'info>,
}

pub fn handle_update_config(
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
    let config = &mut ctx.accounts.faucet_config;
    let clock = Clock::get()?;
    let mut field_changed = String::from("multiple");

    if let Some(v) = is_paused {
        config.is_paused = v;
        field_changed = String::from("is_paused");
    }
    if let Some(v) = min_amount { config.min_amount = v; }
    if let Some(v) = max_amount { config.max_amount = v; }
    if let Some(v) = cooldown_tier_amounts { config.cooldown_tier_amounts = v; }
    if let Some(v) = cooldown_tier_seconds { config.cooldown_tier_seconds = v; }
    if let Some(v) = daily_global_limit { config.daily_global_limit = v; }
    if let Some(v) = referral_enabled { config.referral_enabled = v; }
    if let Some(v) = referral_bonus_claimer { config.referral_bonus_claimer = v; }
    if let Some(v) = referral_bonus_referrer { config.referral_bonus_referrer = v; }
    if let Some(v) = new_authority { config.authority = v; }

    emit!(ConfigUpdatedEvent {
        authority: ctx.accounts.authority.key(),
        timestamp: clock.unix_timestamp,
        field_changed,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 7: BLOCK WALLET
// Admin bans or unbans a wallet.
// ============================================================

#[derive(Accounts)]
#[instruction(target_wallet: Pubkey)]
pub struct BlockWallet<'info> {
    #[account(
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
        has_one = authority @ AfrodevsError::Unauthorized,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    #[account(
        mut,
        seeds = [CLAIMER_SEED, target_wallet.as_ref()],
        bump = claimer_record.bump,
    )]
    pub claimer_record: Account<'info, ClaimerRecord>,

    pub authority: Signer<'info>,
}

pub fn handle_block_wallet(
    ctx: Context<BlockWallet>,
    target_wallet: Pubkey,
    block: bool,
) -> Result<()> {
    ctx.accounts.claimer_record.is_blocked = block;

    emit!(WalletBlockedEvent {
        target_wallet,
        is_blocked: block,
        authority: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 8: WITHDRAW TREASURY
// Admin emergency fund recovery.
// ============================================================

#[derive(Accounts)]
pub struct WithdrawTreasury<'info> {
    #[account(
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
        has_one = authority @ AfrodevsError::Unauthorized,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    /// CHECK: Treasury vault PDA
    #[account(
        mut,
        seeds = [TREASURY_VAULT_SEED],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_withdraw_treasury(
    ctx: Context<WithdrawTreasury>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, AfrodevsError::InvalidAmount);

    let treasury_balance = ctx.accounts.treasury_vault.lamports();
    require!(
        treasury_balance >= amount + RENT_RESERVE_LAMPORTS,
        AfrodevsError::RentReserveViolation
    );

    **ctx.accounts.treasury_vault.try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.authority.try_borrow_mut_lamports()? += amount;

    let new_balance = ctx.accounts.treasury_vault.lamports();

    emit!(WithdrawalEvent {
        authority: ctx.accounts.authority.key(),
        amount,
        destination: ctx.accounts.authority.key(),
        new_balance,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ============================================================
// INSTRUCTION 9: CLOSE CLAIMER RECORD
// Cleanup. Admin or user themselves.
// ============================================================

#[derive(Accounts)]
#[instruction(target_wallet: Pubkey)]
pub struct CloseClaimerRecord<'info> {
    #[account(
        seeds = [FAUCET_CONFIG_SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    #[account(
        mut,
        close = rent_receiver,
        seeds = [CLAIMER_SEED, target_wallet.as_ref()],
        bump = claimer_record.bump,
    )]
    pub claimer_record: Account<'info, ClaimerRecord>,

    /// CHECK: receives the rent lamports
    #[account(mut)]
    pub rent_receiver: AccountInfo<'info>,

    pub signer: Signer<'info>,
}

pub fn handle_close_claimer_record(
    ctx: Context<CloseClaimerRecord>,
    target_wallet: Pubkey,
) -> Result<()> {
    let is_authority = ctx.accounts.signer.key() == ctx.accounts.faucet_config.authority;
    let is_owner = ctx.accounts.signer.key() == target_wallet;

    require!(is_authority || is_owner, AfrodevsError::Unauthorized);

    require!(
        ctx.accounts.claimer_record.pending_referral_bonus == 0,
        AfrodevsError::NoPendingBonus
    );

    Ok(())
}