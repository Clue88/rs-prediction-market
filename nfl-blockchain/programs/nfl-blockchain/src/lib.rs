use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

declare_id!("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc");

#[program]
pub mod nfl_blockchain {
    use super::*;

    /// Create a new binary market.
    ///
    /// This:
    /// - creates the Market account
    /// - creates YES and NO SPL mints
    /// - creates a collateral vault for the market
    /// - wires all of them to a PDA authority
    pub fn create_market(ctx: Context<CreateMarket>, expiry_ts: i64) -> Result<()> {
        require!(expiry_ts > 0, NflError::InvalidExpiry);

        let market = &mut ctx.accounts.market;

        market.authority = ctx.accounts.authority.key();
        market.base_mint = ctx.accounts.base_mint.key();
        market.yes_mint = ctx.accounts.yes_mint.key();
        market.no_mint = ctx.accounts.no_mint.key();
        market.vault = ctx.accounts.vault.key();
        market.expiry_ts = expiry_ts;
        market.status = MarketStatus::Open;
        market.outcome = Outcome::Pending;
        market.market_authority_bump = ctx.bumps.market_authority;

        msg!(
            "Market created: market={}, base_mint={}, yes_mint={}, no_mint={}, vault={}, expiry_ts={}",
            market.key(),
            market.base_mint,
            market.yes_mint,
            market.no_mint,
            market.vault,
            expiry_ts
        );

        Ok(())
    }

    /// Mint YES/NO pairs:
    ///
    /// - User deposits `amount` units of collateral into the market vault
    /// - Program mints `amount` YES tokens to user
    /// - Program mints `amount` NO tokens to user
    pub fn mint_pairs(ctx: Context<MintPairs>, amount: u64) -> Result<()> {
        require!(amount > 0, NflError::InvalidAmount);

        let market = &ctx.accounts.market;
        require!(market.status == MarketStatus::Open, NflError::MarketNotOpen);

        // On-chain keys must match Market config
        require_keys_eq!(
            market.base_mint,
            ctx.accounts.base_mint.key(),
            NflError::InvalidBaseMint
        );
        require_keys_eq!(
            market.yes_mint,
            ctx.accounts.yes_mint.key(),
            NflError::InvalidYesMint
        );
        require_keys_eq!(
            market.no_mint,
            ctx.accounts.no_mint.key(),
            NflError::InvalidNoMint
        );
        require_keys_eq!(
            market.vault,
            ctx.accounts.vault.key(),
            NflError::InvalidVault
        );

        // Transfer collateral from user to market vault
        {
            let cpi_accounts = token::Transfer {
                from: ctx.accounts.user_collateral_ata.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, amount)?;
        }

        // PDA seeds for market_authority (the mint/vault authority)
        let market_key = market.key();
        let signer_seeds: &[&[u8]] = &[
            b"market_auth",
            market_key.as_ref(),
            &[market.market_authority_bump],
        ];
        let signer: &[&[&[u8]]] = &[signer_seeds];

        // Mint YES to user
        {
            let cpi_accounts = token::MintTo {
                mint: ctx.accounts.yes_mint.to_account_info(),
                to: ctx.accounts.user_yes_ata.to_account_info(),
                authority: ctx.accounts.market_authority.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer,
            );
            token::mint_to(cpi_ctx, amount)?;
        }

        // Mint NO to user
        {
            let cpi_accounts = token::MintTo {
                mint: ctx.accounts.no_mint.to_account_info(),
                to: ctx.accounts.user_no_ata.to_account_info(),
                authority: ctx.accounts.market_authority.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer,
            );
            token::mint_to(cpi_ctx, amount)?;
        }

        msg!(
            "Minted {} YES/NO pairs for user {} in market {}",
            amount,
            ctx.accounts.user.key(),
            market.key()
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

    /// YES SPL mint for this market.
    pub yes_mint: Pubkey,

    /// NO SPL mint for this market.
    pub no_mint: Pubkey,

    /// Collateral vault token account (base_mint) for this market.
    pub vault: Pubkey,

    /// UNIX timestamp after which the market can be resolved.
    pub expiry_ts: i64,

    /// Trading lifecycle status.
    pub status: MarketStatus,

    /// Final event outcome (pending until resolved).
    pub outcome: Outcome,

    /// Bump for the market_authority PDA.
    pub market_authority_bump: u8,
}

impl Market {
    /// Size of the Market account (excluding 8-byte discriminator)
    pub const SIZE: usize =
          32   // authority
        + 32   // base_mint
        + 32   // yes_mint
        + 32   // no_mint
        + 32   // vault
        + 8    // expiry_ts
        + 1    // status
        + 1    // outcome
        + 1    // market_authority_bump
    ;
}

/// Create a market, YES/NO mints, and vault.
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
    pub base_mint: Account<'info, Mint>,

    /// YES SPL mint for this market.
    #[account(
        init,
        payer = authority,
        mint::decimals = base_mint.decimals,
        mint::authority = market_authority
    )]
    pub yes_mint: Account<'info, Mint>,

    /// NO SPL mint for this market.
    #[account(
        init,
        payer = authority,
        mint::decimals = base_mint.decimals,
        mint::authority = market_authority
    )]
    pub no_mint: Account<'info, Mint>,

    /// Vault token account that holds collateral for this market.
    #[account(
        init,
        payer = authority,
        token::mint = base_mint,
        token::authority = market_authority
    )]
    pub vault: Account<'info, TokenAccount>,

    /// PDA that acts as authority for mints and vault.
    #[account(
        seeds = [b"market_auth", market.key().as_ref()],
        bump
    )]
    pub market_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

    /// Needed by init for rent-exempt accounts.
    pub rent: Sysvar<'info, Rent>,
}

/// Mints YES/NO pairs backed by deposited collateral.
#[derive(Accounts)]
pub struct MintPairs<'info> {
    /// User minting YES/NO exposure.
    #[account(mut)]
    pub user: Signer<'info>,

    /// User's collateral account (base_mint).
    #[account(
        mut,
        constraint = user_collateral_ata.owner == user.key(),
        constraint = user_collateral_ata.mint == market.base_mint
    )]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    /// Market configuration.
    #[account(
        mut,
        has_one = base_mint,
        has_one = yes_mint,
        has_one = no_mint,
        has_one = vault
    )]
    pub market: Account<'info, Market>,

    /// Collateral mint (matches market.base_mint).
    pub base_mint: Account<'info, Mint>,

    /// YES mint (matches market.yes_mint).
    #[account(mut)]
    pub yes_mint: Account<'info, Mint>,

    /// NO mint (matches market.no_mint).
    #[account(mut)]
    pub no_mint: Account<'info, Mint>,

    /// Market's collateral vault.
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    /// User's YES token account.
    #[account(
        mut,
        constraint = user_yes_ata.owner == user.key(),
        constraint = user_yes_ata.mint == yes_mint.key()
    )]
    pub user_yes_ata: Account<'info, TokenAccount>,

    /// User's NO token account.
    #[account(
        mut,
        constraint = user_no_ata.owner == user.key(),
        constraint = user_no_ata.mint == no_mint.key()
    )]
    pub user_no_ata: Account<'info, TokenAccount>,

    /// PDA authority used to mint YES/NO and control the vault.
    #[account(
        seeds = [b"market_auth", market.key().as_ref()],
        bump = market.market_authority_bump
    )]
    /// CHECK: PDA authority, no data.
    pub market_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
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
    #[msg("Mint amount must be positive.")]
    InvalidAmount,
    #[msg("Market is not open for minting.")]
    MarketNotOpen,
    #[msg("Base mint does not match market config.")]
    InvalidBaseMint,
    #[msg("YES mint does not match market config.")]
    InvalidYesMint,
    #[msg("NO mint does not match market config.")]
    InvalidNoMint,
    #[msg("Vault account does not match market config.")]
    InvalidVault,
}
