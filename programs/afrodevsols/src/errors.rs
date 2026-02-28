// programs/afrodevsols/src/errors.rs

use anchor_lang::prelude::*;

#[error_code]
pub enum AfrodevsError {
    #[msg("The faucet is currently paused. Check back soon!")]
    FaucetPaused,

    #[msg("Treasury balance is too low. Faucet needs refilling.")]
    InsufficientTreasury,

    #[msg("Your cooldown for this tier is still active. Wait a bit!")]
    CooldownActive,

    #[msg("This wallet has been blocked from the faucet.")]
    WalletBlocked,

    #[msg("The daily global distribution limit has been reached.")]
    DailyLimitReached,

    #[msg("That amount is not a valid claim tier.")]
    InvalidAmount,

    #[msg("Amount is below the minimum allowed.")]
    AmountTooLow,

    #[msg("Amount is above the maximum allowed.")]
    AmountTooHigh,

    #[msg("You are not authorized to perform this action.")]
    Unauthorized,

    #[msg("Invalid referral code provided.")]
    InvalidReferral,

    #[msg("You cannot refer yourself.")]
    SelfReferral,

    #[msg("This wallet already exists — referral bonus only applies to new users.")]
    ReferralAlreadyUsed,

    #[msg("No pending referral bonus to collect.")]
    NoPendingBonus,

    #[msg("This withdrawal would drain the treasury below the rent reserve.")]
    RentReserveViolation,

    #[msg("The referral program is currently disabled.")]
    ReferralDisabled,

    #[msg("Recipient list cannot be empty.")]
    EmptyRecipientList,

    #[msg("Recipients and amounts arrays must be the same length.")]
    RecipientAmountMismatch,

    #[msg("Too many recipients. Maximum is 20 per bulk grant.")]
    TooManyRecipients,

    #[msg("A reason label is required for admin grants.")]
    EmptyReason,

    #[msg("Batch total would drain treasury below rent reserve.")]
    BatchTooLarge,

    #[msg("Arithmetic overflow occurred.")]
    Overflow,
}