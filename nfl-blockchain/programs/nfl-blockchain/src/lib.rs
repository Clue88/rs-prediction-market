//! # NFL Blockchain
//!
//! This program implements a binary prediction market on Solana using the Anchor framework.
//! It allows for the creation of markets, minting of YES/NO token pairs backed by collateral,
//! resolution of markets to final outcomes, and redemption of winning tokens for collateral.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo};

declare_id!("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc");

// --- Instruction Data Structs ---

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MarketBuyParams {
    pub quantity: u64,
    pub want_yes: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BuyExactParams {
    pub max_price: u64,
    pub quantity: u64,
    pub want_yes: bool,
}

/// NFL Blockchain program.
#[program]
pub mod nfl_blockchain {
    use super::*;

    pub fn create_market(ctx: Context<CreateMarket>, expiry_ts: i64) -> Result<()> {
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
        Ok(())
    }

    pub fn initialize_order_book(ctx: Context<InitializeOrderBook>) -> Result<()> {
        let ob = &mut ctx.accounts.order_book;
        ob.market = ctx.accounts.market.key();
        ob.next_order_id = 0;
        ob.capacity = 100;
        Ok(())
    }

    pub fn mint_pairs(ctx: Context<MintPairs>, amount: u64) -> Result<()> {
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_collateral_ata.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts), amount)?;

        let market_key = ctx.accounts.market.key();
        let seeds = &[
            b"market_auth",
            market_key.as_ref(),
            &[ctx.accounts.market.market_authority_bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_mint_yes = MintTo {
            mint: ctx.accounts.yes_mint.to_account_info(),
            to: ctx.accounts.user_yes_ata.to_account_info(),
            authority: ctx.accounts.market_authority.to_account_info(),
        };
        token::mint_to(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_mint_yes, signer), amount)?;

        let cpi_mint_no = MintTo {
            mint: ctx.accounts.no_mint.to_account_info(),
            to: ctx.accounts.user_no_ata.to_account_info(),
            authority: ctx.accounts.market_authority.to_account_info(),
        };
        token::mint_to(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_mint_no, signer), amount)?;

        Ok(())
    }

    pub fn place_limit_sell(ctx: Context<PlaceLimitSell>, price: u64, quantity: u64, is_yes: bool) -> Result<()> {
        let (from_account, to_vault) = if is_yes {
            (&ctx.accounts.seller_token_ata, &ctx.accounts.yes_vault)
        } else {
            (&ctx.accounts.seller_token_ata, &ctx.accounts.no_vault)
        };

        let cpi_accounts = Transfer {
            from: from_account.to_account_info(),
            to: to_vault.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts), quantity)?;

        let ob = &mut ctx.accounts.order_book;
        let order_id = ob.next_order_id;
        ob.next_order_id = order_id.checked_add(1).unwrap();

        ob.orders.push(Order {
            id: order_id,
            owner: ctx.accounts.seller.key(),
            seller_receive_collateral_ata: ctx.accounts.seller_receive_collateral_ata.key(),
            price,
            quantity,
            is_yes,
        });
        Ok(())
    }

    pub fn market_buy<'info>(
        ctx: Context<'_, '_, '_, 'info, MarketBuyAccounts<'info>>, 
        params: MarketBuyParams
    ) -> Result<()> {
        let mut quantity_to_buy = params.quantity;
        let want_yes = params.want_yes;
        let ob = &mut ctx.accounts.order_book;
        let mut remaining_iter = ctx.remaining_accounts.iter();

        let market_key = ctx.accounts.market.key();
        let seeds = &[
            b"market_auth",
            market_key.as_ref(),
            &[ctx.accounts.market.market_authority_bump],
        ];
        let signer = &[&seeds[..]];

        let mut i = 0;
        while i < ob.orders.len() && quantity_to_buy > 0 {
            let order = &mut ob.orders[i];

            if order.is_yes != want_yes { i += 1; continue; }
            if order.quantity == 0 { ob.orders.swap_remove(i); continue; }

            let fill_amount = order.quantity.min(quantity_to_buy);
            let seller_collateral_ata_info = remaining_iter.next().ok_or(NflError::MissingSellerAccounts)?;

            if seller_collateral_ata_info.key() != order.seller_receive_collateral_ata {
                return err!(NflError::SellerAccountMismatch);
            }

            let cost = (order.price as u64).checked_mul(fill_amount).unwrap();

            let cpi_pay = Transfer {
                from: ctx.accounts.buyer_collateral_ata.to_account_info(),
                to: seller_collateral_ata_info.clone(),
                authority: ctx.accounts.buyer.to_account_info(),
            };
            token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_pay), cost)?;

            let vault = if want_yes { ctx.accounts.yes_vault.to_account_info() } else { ctx.accounts.no_vault.to_account_info() };
            let cpi_receive = Transfer {
                from: vault,
                to: ctx.accounts.buyer_receive_token_ata.to_account_info(),
                authority: ctx.accounts.market_authority.to_account_info(),
            };
            token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_receive, signer), fill_amount)?;

            order.quantity -= fill_amount;
            quantity_to_buy -= fill_amount;

            if order.quantity == 0 { ob.orders.swap_remove(i); } else { i += 1; }
        }
        Ok(())
    }

    pub fn buy_exact<'info>(
        ctx: Context<'_, '_, '_, 'info, MarketBuyAccounts<'info>>, 
        params: BuyExactParams
    ) -> Result<()> {
        let mut quantity_to_buy = params.quantity;
        let want_yes = params.want_yes;
        let ob = &mut ctx.accounts.order_book;
        let mut remaining_iter = ctx.remaining_accounts.iter();

        let mut needed = quantity_to_buy;
        for order in ob.orders.iter() {
            if order.is_yes != want_yes { continue; }
            if needed == 0 { break; }
            if order.price > params.max_price { return err!(NflError::TooExpensive); }
            needed = needed.saturating_sub(order.quantity);
        }
        if needed > 0 { return err!(NflError::InsufficientLiquidity); }

        let market_key = ctx.accounts.market.key();
        let seeds = &[
            b"market_auth",
            market_key.as_ref(),
            &[ctx.accounts.market.market_authority_bump],
        ];
        let signer = &[&seeds[..]];

        let mut i = 0;
        while i < ob.orders.len() && quantity_to_buy > 0 {
            let order = &mut ob.orders[i];
            if order.is_yes != want_yes { i += 1; continue; }
            if order.quantity == 0 { ob.orders.swap_remove(i); continue; }

            let fill_amount = order.quantity.min(quantity_to_buy);
            let seller_collateral_ata_info = remaining_iter.next().ok_or(NflError::MissingSellerAccounts)?;

            let cost = (order.price as u64).checked_mul(fill_amount).unwrap();

            let cpi_pay = Transfer {
                from: ctx.accounts.buyer_collateral_ata.to_account_info(),
                to: seller_collateral_ata_info.clone(),
                authority: ctx.accounts.buyer.to_account_info(),
            };
            token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_pay), cost)?;

            let vault = if want_yes { ctx.accounts.yes_vault.to_account_info() } else { ctx.accounts.no_vault.to_account_info() };
            let cpi_receive = Transfer {
                from: vault,
                to: ctx.accounts.buyer_receive_token_ata.to_account_info(),
                authority: ctx.accounts.market_authority.to_account_info(),
            };
            token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_receive, signer), fill_amount)?;

            order.quantity -= fill_amount;
            quantity_to_buy -= fill_amount;

            if order.quantity == 0 { ob.orders.swap_remove(i); } else { i += 1; }
        }
        Ok(())
    }

    /// Resolve a market to a final outcome (Yes / No / Invalid).
    ///
    /// - Only `market.authority` may resolve.
    /// - Must be called after `expiry_ts`.
    /// - Sets `market.status = Resolved` and `market.outcome`.
    pub fn resolve_market(ctx: Context<ResolveMarket>, outcome: Outcome) -> Result<()> {
        let market = &mut ctx.accounts.market;

        // Must not already be resolved
        require!(
            market.status != MarketStatus::Resolved,
            NflError::MarketAlreadyResolved
        );

        // Must be past expiry
        let now = Clock::get()?.unix_timestamp;
        require!(now >= market.expiry_ts, NflError::MarketNotExpired);

        // Only allow resolution to a non-pending outcome
        require!(
            matches!(outcome, Outcome::Yes | Outcome::No | Outcome::Invalid),
            NflError::InvalidResolutionOutcome
        );

        market.status = MarketStatus::Resolved;
        market.outcome = outcome;

        msg!(
            "Market {} resolved to {:?} at unix_ts={}",
            market.key(),
            outcome,
            now
        );

        Ok(())
    }

    /// Redeem winning YES/NO tokens for collateral.
    ///
    /// - Market must be `Resolved`.
    /// - If outcome = YES: burns user's YES and sends USDC from vault.
    /// - If outcome = NO: burns user's NO and sends USDC from vault.
    /// - Outcome = Invalid / Pending: not redeemable.
    pub fn redeem(ctx: Context<Redeem>) -> Result<()> {
        let market = &ctx.accounts.market;

        // Market must be resolved
        require!(
            market.status == MarketStatus::Resolved,
            NflError::MarketNotResolved
        );

        // Choose the winning mint and ATA based on outcome
        let (winner_mint_ai, winner_ata_ai, winner_amount) = match market.outcome {
            Outcome::Yes => (
                ctx.accounts.yes_mint.to_account_info(),
                ctx.accounts.user_yes_ata.to_account_info(),
                ctx.accounts.user_yes_ata.amount,
            ),
            Outcome::No => (
                ctx.accounts.no_mint.to_account_info(),
                ctx.accounts.user_no_ata.to_account_info(),
                ctx.accounts.user_no_ata.amount,
            ),
            Outcome::Pending | Outcome::Invalid => {
                return err!(NflError::CannotRedeemForOutcome);
            }
        };

        // Nothing to redeem
        require!(winner_amount > 0, NflError::NothingToRedeem);

        // Burn the winning tokens
        {
            let burn_accounts = token::Burn {
                mint: winner_mint_ai.clone(),
                from: winner_ata_ai.clone(),
                authority: ctx.accounts.user.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), burn_accounts);
            token::burn(cpi_ctx, winner_amount)?;
        }

        // Transfer collateral from vault to user, signed by PDA
        let market_key = market.key();
        let signer_seeds: &[&[u8]] = &[
            b"market_auth",
            market_key.as_ref(),
            &[market.market_authority_bump],
        ];
        let signer_seeds: &[&[&[u8]]] = &[signer_seeds];

        let transfer_accounts = token::Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.user_collateral_ata.to_account_info(),
            authority: ctx.accounts.market_authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, winner_amount)?;

        msg!(
            "Redeemed {} units of {:?} for user {} in market {}",
            winner_amount,
            market.outcome,
            ctx.accounts.user.key(),
            market.key()
        );

        Ok(())
    }
}

// --- Accounts ---

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + Market::SIZE)]
    pub market: Account<'info, Market>,
    pub base_mint: Account<'info, Mint>,
    #[account(init, payer = authority, mint::decimals = base_mint.decimals, mint::authority = market_authority)]
    pub yes_mint: Account<'info, Mint>,
    #[account(init, payer = authority, mint::decimals = base_mint.decimals, mint::authority = market_authority)]
    pub no_mint: Account<'info, Mint>,
    #[account(init, payer = authority, token::mint = base_mint, token::authority = market_authority)]
    pub vault: Account<'info, TokenAccount>,
    #[account(seeds = [b"market_auth", market.key().as_ref()], bump)]
    /// CHECK: PDA
    pub market_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeOrderBook<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + 32 + 8 + 8 + 4 + (89 * 100), seeds = [b"orderbook", market.key().as_ref()], bump)]
    pub order_book: Account<'info, OrderBook>,
    pub market: Account<'info, Market>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintPairs<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, constraint = user_collateral_ata.mint == market.base_mint)]
    pub user_collateral_ata: Account<'info, TokenAccount>,
    pub market: Account<'info, Market>,
    pub base_mint: Account<'info, Mint>,
    #[account(mut)]
    pub yes_mint: Account<'info, Mint>,
    #[account(mut)]
    pub no_mint: Account<'info, Mint>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, constraint = user_yes_ata.mint == yes_mint.key())]
    pub user_yes_ata: Account<'info, TokenAccount>,
    #[account(mut, constraint = user_no_ata.mint == no_mint.key())]
    pub user_no_ata: Account<'info, TokenAccount>,
    #[account(seeds = [b"market_auth", market.key().as_ref()], bump = market.market_authority_bump)]
    /// CHECK: PDA
    pub market_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct PlaceLimitSell<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,
    #[account(mut)]
    pub seller_token_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub seller_receive_collateral_ata: Account<'info, TokenAccount>,
    
    // ADDED: We must pass the PDA to check ownership
    #[account(seeds = [b"market_auth", market.key().as_ref()], bump = market.market_authority_bump)]
    /// CHECK: PDA used for constraints
    pub market_authority: UncheckedAccount<'info>,

    // FIXED: Check against market_authority.key()
    #[account(mut, constraint = yes_vault.owner == market_authority.key())]
    pub yes_vault: Account<'info, TokenAccount>,
    #[account(mut, constraint = no_vault.owner == market_authority.key())]
    pub no_vault: Account<'info, TokenAccount>,
    
    #[account(mut, seeds = [b"orderbook", market.key().as_ref()], bump)]
    pub order_book: Account<'info, OrderBook>,
    pub market: Account<'info, Market>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct MarketBuyAccounts<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut, constraint = buyer_collateral_ata.mint == market.base_mint)]
    pub buyer_collateral_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub buyer_receive_token_ata: Account<'info, TokenAccount>,
    pub market: Account<'info, Market>,
    #[account(mut, seeds = [b"orderbook", market.key().as_ref()], bump)]
    pub order_book: Account<'info, OrderBook>,
    #[account(mut)]
    pub yes_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub no_vault: Account<'info, TokenAccount>,
    #[account(seeds = [b"market_auth", market.key().as_ref()], bump = market.market_authority_bump)]
    /// CHECK: PDA
    pub market_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct Market {
    pub authority: Pubkey,
    pub base_mint: Pubkey,
    pub yes_mint: Pubkey,
    pub no_mint: Pubkey,
    pub vault: Pubkey,
    pub expiry_ts: i64,
    pub status: MarketStatus,
    pub outcome: Outcome,
    pub market_authority_bump: u8,
}

impl Market {
    pub const SIZE: usize = 32 + 32 + 32 + 32 + 32 + 8 + 1 + 1 + 1;
}

#[account]
pub struct OrderBook {
    pub market: Pubkey,
    pub next_order_id: u64,
    pub capacity: u64,
    pub orders: Vec<Order>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Order {
    pub id: u64,
    pub owner: Pubkey,
    pub seller_receive_collateral_ata: Pubkey,
    pub price: u64,
    pub quantity: u64,
    pub is_yes: bool,
}

/// Resolve a market to a final outcome.
#[derive(Accounts)]
pub struct ResolveMarket<'info> {
    /// Must match `market.authority`.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The market being resolved.
    #[account(
        mut,
        has_one = authority
    )]
    pub market: Account<'info, Market>,
}

/// Redeem winning YES/NO tokens for collateral.
#[derive(Accounts)]
pub struct Redeem<'info> {
    /// User redeeming their winning tokens.
    #[account(mut)]
    pub user: Signer<'info>,

    /// Market config; ties everything together.
    #[account(
        mut,
        has_one = base_mint,
        has_one = yes_mint,
        has_one = no_mint,
        has_one = vault
    )]
    pub market: Account<'info, Market>,

    /// Collateral mint (e.g. USDC).
    pub base_mint: Account<'info, Mint>,

    /// YES mint.
    #[account(mut)]
    pub yes_mint: Account<'info, Mint>,

    /// NO mint.
    #[account(mut)]
    pub no_mint: Account<'info, Mint>,

    /// Market's collateral vault (holds base_mint).
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    /// User's collateral account (will receive redeemed USDC).
    #[account(
        mut,
        constraint = user_collateral_ata.owner == user.key(),
        constraint = user_collateral_ata.mint == base_mint.key(),
    )]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    /// User's YES token account.
    #[account(
        mut,
        constraint = user_yes_ata.owner == user.key(),
        constraint = user_yes_ata.mint == yes_mint.key(),
    )]
    pub user_yes_ata: Account<'info, TokenAccount>,

    /// User's NO token account.
    #[account(
        mut,
        constraint = user_no_ata.owner == user.key(),
        constraint = user_no_ata.mint == no_mint.key(),
    )]
    pub user_no_ata: Account<'info, TokenAccount>,

    /// PDA that controls vault + mints.
    #[account(
        seeds = [b"market_auth", market.key().as_ref()],
        bump = market.market_authority_bump
    )]
    /// CHECK: PDA authority, no data.
    pub market_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketStatus { Open, Halted, Resolved }

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Pending, Yes, No, Invalid }

#[error_code]
pub enum NflError {
    #[msg("Invalid amount")] InvalidAmount,
    #[msg("Market is not open")] MarketNotOpen,
    #[msg("Order book is full")] OrderBookFull,
    #[msg("Invalid base mint")] InvalidBaseMint,
    #[msg("Invalid yes mint")] InvalidYesMint,
    #[msg("Invalid no mint")] InvalidNoMint,
    #[msg("Invalid vault")] InvalidVault,
    #[msg("Missing seller accounts in remaining_accounts")] MissingSellerAccounts,
    #[msg("Math overflow")] MathOverflow,
    #[msg("Seller account mismatch")] SellerAccountMismatch,
    #[msg("Too expensive")] TooExpensive,
    #[msg("Invalid expiry timestamp")] InvalidExpiry,
    #[msg("Insufficient liquidity to fill order")] InsufficientLiquidity,
    #[msg("Market has already been resolved.")]
    MarketAlreadyResolved,
    #[msg("Market has not yet reached expiry_ts.")]
    MarketNotExpired,
    #[msg("Market is not in a resolved state.")]
    MarketNotResolved,
    #[msg("Invalid outcome supplied for resolution.")]
    InvalidResolutionOutcome,
    #[msg("Cannot redeem in current market outcome.")]
    CannotRedeemForOutcome,
    #[msg("User has no winning tokens to redeem.")]
    NothingToRedeem,
}