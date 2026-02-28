use anchor_lang::prelude::*;

declare_id!("5UHiP59UBysX4yhJ3pdsdVK2QV6wtjAfB6RsZqztWZiL");

#[program]
pub mod afrodevsols {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
