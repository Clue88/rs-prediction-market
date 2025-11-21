use anchor_lang::prelude::*;

declare_id!("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc");

#[program]
pub mod nfl_blockchain {
    use super::*;

    /// Create a new binary market.
    pub fn create_market(ctx: Context<CreateMarket>, expiry_ts: i64) -> Result<()> {
        require!(expiry_ts > 0, NflError::InvalidExpiry);

        let market = &mut ctx.accounts.market;

        market.authority = ctx.accounts.authority.key();
        market.base_mint = ctx.accounts.base_mint.key();
        market.expiry_ts = expiry_ts;
        market.status = MarketStatus::Open;
        market.outcome = Outcome::Pending;

        msg!(
            "Market created with mint {:?}, expiry {}",
            market.base_mint,
            expiry_ts
        );

        Ok(())
    }
}

/// On-chain state for a single binary contract market.
#[account]
pub struct Market {
    /// Authority (exchange admin or oracle setter).
    pub authority: Pubkey,

    /// Collateral mint used for payouts (e.g., USDC).
    pub base_mint: Pubkey,

    /// UNIX timestamp after which the market can be resolved.
    pub expiry_ts: i64,

    /// Trading lifecycle status.
    pub status: MarketStatus,

    /// Final event outcome (pending until resolved).
    pub outcome: Outcome,
}

impl Market {
    /// Size of the Market account (excluding 8-byte discriminator)
    pub const SIZE: usize = 32   // authority
        + 32   // base_mint
        + 8    // expiry_ts
        + 1    // status enum
        + 1    // outcome enum
    ;
}

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    /// Exchange admin creating the market.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The Market account to initialize.
    #[account(
        init,
        payer = authority,
        space = 8 + Market::SIZE
    )]
    pub market: Account<'info, Market>,

    /// Collateral mint for this market (e.g. USDC).
    pub base_mint: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MarketStatus {
    /// Market open.
    Open,
    /// Trading halted but not settled.
    Halted,
    /// Market resolved; settlement allowed.
    Resolved,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No resolution yet.
    Pending,
    /// YES is the winning side.
    Yes,
    /// NO is the winning side.
    No,
    /// Invalid: event ambiguous or nullified.
    Invalid,
}

#[error_code]
pub enum NflError {
    #[msg("Expiry timestamp must be positive.")]
    InvalidExpiry,
}
