use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer}; // Removed unused MintTo

declare_id!("2qdp2bKXQHhRiD1kPS22Zyx3dxuevXkiRgvWKghHSGzx");

// Const function to parse u64 from string at compile time
const fn parse_u64_from_str(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut result = 0u64;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b >= b'0' && b <= b'9' {
            result = result * 10 + (b - b'0') as u64;
        } else {
            // Invalid character, return default of 1
            return 1;
        }
        i += 1;
    }
    result
}

// Price scale for fractional prices: defaults to 1 if PRICE_SCALE env var is not set
// Can be set at compile time via environment variable: PRICE_SCALE=1000000000 cargo build
// Example: price = 1_500_000_000 represents 1.5, price = 500_000_000 represents 0.5
pub const PRICE_SCALE: u64 = {
    const ENV_STR: Option<&str> = option_env!("PRICE_SCALE");
    match ENV_STR {
        Some(s) => parse_u64_from_str(s),
        None => 1,
    }
};

// --- Instruction Data Structs ---

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MarketBuyParams {
    pub quantity: u64,
    pub want_yes: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BuyExactParams {
    /// Maximum price per unit as scaled integer: max_price = actual_price * PRICE_SCALE
    pub max_price: u64,
    pub quantity: u64,
    pub want_yes: bool,
}

/// NFL Blockchain program.
#[program]
pub mod nfl_blockchain {
    use super::*;

    /// Create a new binary market.
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

    /// Mint YES/NO pairs.
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

        // PDA seeds for market_authority
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

    /// Resolve a market to a final outcome.
    pub fn resolve_market(ctx: Context<ResolveMarket>, outcome: Outcome) -> Result<()> {
        let market = &mut ctx.accounts.market;

        require!(
            market.status != MarketStatus::Resolved,
            NflError::MarketAlreadyResolved
        );

        let now = Clock::get()?.unix_timestamp;
        require!(now >= market.expiry_ts, NflError::MarketNotExpired);

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
    pub fn redeem(ctx: Context<Redeem>) -> Result<()> {
        let market = &ctx.accounts.market;

        require!(
            market.status == MarketStatus::Resolved,
            NflError::MarketNotResolved
        );

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

    // -------------------------------------------------------------------------
    // NEW: ORDER BOOK FUNCTIONALITY
    // -------------------------------------------------------------------------

    /// Initialize a new OrderBook account.
    pub fn initialize_order_book(ctx: Context<InitializeOrderBook>) -> Result<()> {
        let ob = &mut ctx.accounts.order_book;
        // Link this order book to the specific market it serves
        ob.market = ctx.accounts.market.key();
        ob.next_order_id = 0;
        ob.capacity = 100;
        msg!("Order Book initialized for Market: {}", ctx.accounts.market.key());
        Ok(())
    }

    /// Place a Limit Sell Order.
    /// This escrows the Seller's outcome tokens (YES or NO) into the vault 
    /// and records their desire to sell at a specific price.
    /// 
    /// Price is stored as a scaled integer: price = actual_price * PRICE_SCALE
    /// Example: price = 1_500_000_000 represents 1.5, price = 500_000_000 represents 0.5
    /// When calculating payment, the cost is rounded down: cost = (price * quantity) / PRICE_SCALE
    pub fn place_limit_sell(ctx: Context<PlaceLimitSell>, price: u64, quantity: u64, is_yes: bool) -> Result<()> {
        require!(quantity > 0, NflError::InvalidAmount);
        
        // Determine which tokens to escrow (YES tokens or NO tokens)
        // and which vault they should go to.
        let (from_account, to_vault) = if is_yes {
            (&ctx.accounts.seller_token_ata, &ctx.accounts.yes_vault)
        } else {
            (&ctx.accounts.seller_token_ata, &ctx.accounts.no_vault)
        };

        // 1. Escrow Transfer: Move tokens from Seller -> OrderBook Vault
        // This ensures the tokens are available immediately when a buyer arrives.
        let cpi_accounts = Transfer {
            from: from_account.to_account_info(),
            to: to_vault.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts), quantity)?;

        // 2. Update State: Add the order to the on-chain vector
        let ob = &mut ctx.accounts.order_book;
        let order_id = ob.next_order_id;
        ob.next_order_id = order_id.checked_add(1).unwrap();

        // Check capacity to prevent exceeding account size limits
        if ob.orders.len() as u64 >= ob.capacity {
            return err!(NflError::OrderBookFull);
        }

        // Push the order struct. Note: We store the seller's collateral ATA 
        // so we know where to send the USDC when this order is filled.
        ob.orders.push(Order {
            id: order_id,
            owner: ctx.accounts.seller.key(),
            seller_receive_collateral_ata: ctx.accounts.seller_receive_collateral_ata.key(),
            price,
            quantity,
            is_yes,
        });

        msg!("Order Placed: ID={}, Price={}, Qty={}, IsYes={}", order_id, price, quantity, is_yes);
        Ok(())
    }

    /// Market Buy: Fills orders starting from the oldest/best price until quantity is met.
    /// Uses 'remaining_accounts' to pay arbitrary sellers.
    pub fn market_buy<'info>(
        ctx: Context<'_, '_, '_, 'info, MarketBuyAccounts<'info>>, 
        params: MarketBuyParams
    ) -> Result<()> {
        let mut quantity_to_buy = params.quantity;
        let want_yes = params.want_yes;

        // RUST BORROW CHECKER WORKAROUND:
        // We need 'order_book_info' (immutable) for the CPI signer later.
        // But we also need 'ob' (mutable) to modify the orders vector.
        // We must extract the immutable reference *before* taking the mutable borrow.
        let order_book_info = ctx.accounts.order_book.to_account_info();
        let ob = &mut ctx.accounts.order_book;
        
        // Iterator for sellers passed in via 'remaining_accounts'
        let mut remaining_iter = ctx.remaining_accounts.iter();

        // Prepare PDA signer seeds (needed to unlock tokens from the Vault)
        let market_key = ctx.accounts.market.key();
        let bump = ctx.bumps.order_book;
        let seeds = &[
            b"orderbook",
            market_key.as_ref(),
            &[bump],
        ];
        let signer = &[&seeds[..]];

        let mut i = 0;
        // Loop through orders until we satisfy the buy quantity or run out of orders
        while i < ob.orders.len() && quantity_to_buy > 0 {
            let order = &mut ob.orders[i];

            // Skip orders that don't match the side we want (YES vs NO)
            if order.is_yes != want_yes { i += 1; continue; }
            
            // Cleanup: remove empty orders if encountered
            if order.quantity == 0 { ob.orders.swap_remove(i); continue; }

            // Determine how much to fill from this specific order
            let fill_amount = order.quantity.min(quantity_to_buy);

            // Fetch the specific Seller's account from remaining_accounts
            let seller_collateral_ata_info = remaining_iter.next().ok_or(NflError::MissingSellerAccounts)?;

            // SECURITY CHECK: Ensure the account passed matches the order's owner
            if seller_collateral_ata_info.key() != order.seller_receive_collateral_ata {
                return err!(NflError::SellerAccountMismatch);
            }

            // Calculate cost with fractional price support: cost = (price * fill_amount) / PRICE_SCALE
            // Round down by using integer division
            let cost = (order.price as u128)
                .checked_mul(fill_amount as u128)
                .ok_or(NflError::MathOverflow)?
                .checked_div(PRICE_SCALE as u128)
                .ok_or(NflError::MathOverflow)? as u64;

            // 1. Payment Transfer: Buyer pays Seller (Collateral/USDC) directly
            let cpi_pay = Transfer {
                from: ctx.accounts.buyer_collateral_ata.to_account_info(),
                to: seller_collateral_ata_info.clone(),
                authority: ctx.accounts.buyer.to_account_info(),
            };
            token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_pay), cost)?;

            // 2. Asset Transfer: Vault releases Outcome Tokens to Buyer
            // Signed by the OrderBook PDA
            let vault = if want_yes { ctx.accounts.yes_vault.to_account_info() } else { ctx.accounts.no_vault.to_account_info() };
            let cpi_receive = Transfer {
                from: vault,
                to: ctx.accounts.buyer_receive_token_ata.to_account_info(),
                authority: order_book_info.clone(), // Use the immutable reference here
            };
            token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_receive, signer), fill_amount)?;

            // Update state
            order.quantity -= fill_amount;
            quantity_to_buy -= fill_amount;

            // If order is fully filled, remove it. Else, move to next.
            if order.quantity == 0 { ob.orders.swap_remove(i); } else { i += 1; }
        }
        
        Ok(())
    }

    /// Buy Exact: Identical to Market Buy, but verifies liquidity and price constraints FIRST.
    /// This is atomic: if the full quantity cannot be bought under max_price, the transaction fails.
    pub fn buy_exact<'info>(
        ctx: Context<'_, '_, '_, 'info, MarketBuyAccounts<'info>>, 
        params: BuyExactParams
    ) -> Result<()> {
        let mut quantity_to_buy = params.quantity;
        let want_yes = params.want_yes;

        let order_book_info = ctx.accounts.order_book.to_account_info();
        let ob = &mut ctx.accounts.order_book;
        let mut remaining_iter = ctx.remaining_accounts.iter();

        // Check if the trade is possible BEFORE moving any funds.
        let mut needed = quantity_to_buy;
        for order in ob.orders.iter() {
            if order.is_yes != want_yes { continue; }
            if needed == 0 { break; }
            
            // If we hit an order that is too expensive, the whole trade fails
            if order.price > params.max_price { return err!(NflError::TooExpensive); }
            
            needed = needed.saturating_sub(order.quantity);
        }
        // If we processed all orders and still need tokens, fail.
        if needed > 0 { return err!(NflError::InsufficientLiquidity); }

        // Execute Trade (see more details in market_buy above)
        let market_key = ctx.accounts.market.key();
        let bump = ctx.bumps.order_book;
        let seeds = &[
            b"orderbook",
            market_key.as_ref(),
            &[bump],
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

            // Calculate cost with fractional price support: cost = (price * fill_amount) / PRICE_SCALE
            // Round down by using integer division
            let cost = (order.price as u128)
                .checked_mul(fill_amount as u128)
                .unwrap()
                .checked_div(PRICE_SCALE as u128)
                .unwrap() as u64;

            // Buyer pays Seller
            let cpi_pay = Transfer {
                from: ctx.accounts.buyer_collateral_ata.to_account_info(),
                to: seller_collateral_ata_info.clone(),
                authority: ctx.accounts.buyer.to_account_info(),
            };
            token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_pay), cost)?;

            // Vault releases tokens to Buyer
            let vault = if want_yes { ctx.accounts.yes_vault.to_account_info() } else { ctx.accounts.no_vault.to_account_info() };
            let cpi_receive = Transfer {
                from: vault,
                to: ctx.accounts.buyer_receive_token_ata.to_account_info(),
                authority: order_book_info.clone(),
            };
            token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_receive, signer), fill_amount)?;

            order.quantity -= fill_amount;
            quantity_to_buy -= fill_amount;

            if order.quantity == 0 { ob.orders.swap_remove(i); } else { i += 1; }
        }
        Ok(())
    }
}
// --- Accounts ---

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + Market::SIZE
    )]
    pub market: Account<'info, Market>,

    pub base_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        mint::decimals = base_mint.decimals,
        mint::authority = market_authority
    )]
    pub yes_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        mint::decimals = base_mint.decimals,
        mint::authority = market_authority
    )]
    pub no_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = base_mint,
        token::authority = market_authority
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"market_auth", market.key().as_ref()],
        bump
    )]
    /// CHECK: PDA authority, no data.
    pub market_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeOrderBook<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        init, 
        payer = authority, 
        space = 8 + 32 + 8 + 8 + 4 + (89 * 100), 
        seeds = [b"orderbook", market.key().as_ref()], 
        bump
    )]
    pub order_book: Account<'info, OrderBook>,
    
    pub market: Account<'info, Market>,
    
    pub yes_mint: Account<'info, Mint>,
    pub no_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = yes_mint,
        token::authority = order_book,
        seeds = [b"yes_vault", order_book.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        token::mint = no_mint,
        token::authority = order_book,
        seeds = [b"no_vault", order_book.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintPairs<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_collateral_ata.owner == user.key(),
        constraint = user_collateral_ata.mint == market.base_mint
    )]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        has_one = base_mint,
        has_one = yes_mint,
        has_one = no_mint,
        has_one = vault
    )]
    pub market: Account<'info, Market>,

    pub base_mint: Account<'info, Mint>,

    #[account(mut)]
    pub yes_mint: Account<'info, Mint>,

    #[account(mut)]
    pub no_mint: Account<'info, Mint>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_yes_ata.owner == user.key(),
        constraint = user_yes_ata.mint == yes_mint.key()
    )]
    pub user_yes_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_no_ata.owner == user.key(),
        constraint = user_no_ata.mint == no_mint.key()
    )]
    pub user_no_ata: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"market_auth", market.key().as_ref()],
        bump = market.market_authority_bump
    )]
    /// CHECK: PDA authority, no data.
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
    
    #[account(mut, seeds = [b"orderbook", market.key().as_ref()], bump)]
    pub order_book: Account<'info, OrderBook>,
    
    #[account(
        mut, 
        seeds = [b"yes_vault", order_book.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut, 
        seeds = [b"no_vault", order_book.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,
    
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
    
    #[account(
        mut, 
        seeds = [b"yes_vault", order_book.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,
    #[account(
        mut, 
        seeds = [b"no_vault", order_book.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,

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
    /// Price per unit as scaled integer: price = actual_price * PRICE_SCALE
    /// Example: 1_500_000_000 = 1.5, 500_000_000 = 0.5
    pub price: u64,
    pub quantity: u64,
    pub is_yes: bool,
}

#[derive(Accounts)]
pub struct ResolveMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority
    )]
    pub market: Account<'info, Market>,
}

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        has_one = base_mint,
        has_one = yes_mint,
        has_one = no_mint,
        has_one = vault
    )]
    pub market: Account<'info, Market>,

    pub base_mint: Account<'info, Mint>,

    #[account(mut)]
    pub yes_mint: Account<'info, Mint>,

    #[account(mut)]
    pub no_mint: Account<'info, Mint>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_collateral_ata.owner == user.key(),
        constraint = user_collateral_ata.mint == base_mint.key(),
    )]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_yes_ata.owner == user.key(),
        constraint = user_yes_ata.mint == yes_mint.key(),
    )]
    pub user_yes_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_no_ata.owner == user.key(),
        constraint = user_no_ata.mint == no_mint.key(),
    )]
    pub user_no_ata: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"market_auth", market.key().as_ref()],
        bump = market.market_authority_bump
    )]
    /// CHECK: PDA authority, no data.
    pub market_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketStatus { 
    Open, 
    Halted, 
    Resolved 
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { 
    Pending, 
    Yes, 
    No, 
    Invalid 
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
    // Order Book Errors
    #[msg("Order book is full")] 
    OrderBookFull,
    #[msg("Missing seller accounts in remaining_accounts")] 
    MissingSellerAccounts,
    #[msg("Math overflow")] 
    MathOverflow,
    #[msg("Seller account mismatch")] 
    SellerAccountMismatch,
    #[msg("Too expensive")] 
    TooExpensive,
    #[msg("Insufficient liquidity to fill order")] 
    InsufficientLiquidity,
}