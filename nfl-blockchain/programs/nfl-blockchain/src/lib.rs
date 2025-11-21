use anchor_lang::prelude::*;

declare_id!("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc");

#[program]
pub mod nfl_blockchain {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
