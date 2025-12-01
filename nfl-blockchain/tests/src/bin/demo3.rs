//! Demo 3: User 2 buys NO tokens from User 1
//! 
//! This script demonstrates:
//! 1. User 2 sets up collateral account
//! 2. User 2 executes market buy to purchase NO tokens that User 1 is selling (from demo2)
//!
//! Usage:
//!   export ANCHOR_WALLET=~/.config/solana/user2.json  # User 2's wallet
//!   export MARKET=<market_address>  # From demo1 output
//!   export BASE_MINT=<base_mint_address>  # From demo1 output
//!   export YES_MINT=<yes_mint_address>  # From demo1 output
//!   export NO_MINT=<no_mint_address>  # From demo1 output
//!   export VAULT=<vault_address>  # From demo1 output
//!   export MARKET_AUTHORITY=<market_authority_address>  # From demo1 output
//!   export ORDER_BOOK=<order_book_address>  # From demo1 output
//!   export YES_VAULT=<yes_vault_address>  # From demo1 output
//!   export NO_VAULT=<no_vault_address>  # From demo1 output
//!   export USER1_COLLATERAL=<user1_collateral_ata>  # User 1's collateral account (to pay seller)
//!   cargo run --bin demo3

#![allow(deprecated)]

use anchor_client::solana_sdk::{
    instruction::AccountMeta, pubkey::Pubkey, signature::Signer,
};
use anchor_spl::token::TokenAccount;
use std::str::FromStr;

use tests::test_utils::*;

fn main() {
    println!("==========================================");
    println!("Demo 3: User 2");
    println!("Buying NO tokens from User 1...");
    println!("==========================================\n");

    // Check for required environment variables
    let market = get_env_pubkey("MARKET");
    let base_mint = get_env_pubkey("BASE_MINT");
    let _yes_mint = get_env_pubkey("YES_MINT");
    let no_mint = get_env_pubkey("NO_MINT");
    let order_book = get_env_pubkey("ORDER_BOOK");
    let yes_vault = get_env_pubkey("YES_VAULT");
    let no_vault = get_env_pubkey("NO_VAULT");
    let user1_collateral = get_env_pubkey("USER1_COLLATERAL");

    // Setup client as User 2
    println!("Step 1: Setting up client as User 2...");
    let (program, payer) = setup_client();
    println!("   [OK] User 2 address: {}", payer.pubkey());

    // Step 2: Check order book for available NO token sell orders
    println!("\nStep 2: Checking order book for NO token sell orders...");
    let ob_account: nfl_blockchain::OrderBook = program.account(order_book).unwrap();
    println!("   [OK] Order book has {} order(s)", ob_account.orders.len());
    
    // Find NO token sell orders (is_yes = false)
    let no_orders: Vec<_> = ob_account.orders.iter()
        .filter(|o| !o.is_yes)
        .collect();
    
    if no_orders.is_empty() {
        eprintln!("   [ERROR] No NO token sell orders found in order book!");
        eprintln!("   Please ensure User 1 has placed a sell order for NO tokens (run demo2 first).");
        std::process::exit(1);
    }
    
    println!("   [OK] Found {} NO token sell order(s)", no_orders.len());
    for (i, order) in no_orders.iter().enumerate() {
        println!("   Order {}: ID={}, Price={}, Quantity={}", 
                 i + 1, order.id, order.price, order.quantity);
    }

    // Step 3: Create collateral account and fund it
    println!("\nStep 3: Creating and funding collateral account...");
    let user_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    
    // Check if this is wSOL (So11111111111111111111111111111111111111112)
    let wsol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let is_wsol = base_mint == wsol_mint;
    
    // User 2 wagers 2 SOL = 2_000_000_000 lamports
    let wager_amount = 2_000_000_000; // 2 SOL
    
    if is_wsol {
        println!("   [OK] Using wSOL as collateral (no minting needed)");
        println!("   [OK] User 2 collateral account: {}", user_collateral);
        println!("   [INFO] User 2 should already have wSOL from wrapping");
        println!("   [INFO] User 2 needs {} lamports (2 SOL) to buy NO tokens", wager_amount);
    } else {
        // For non-wSOL mints, need to mint tokens
        // Note: This requires the mint authority, which User 2 may not have
        // In a real scenario, Market Authority would mint tokens for User 2, or User 2 would receive them via transfer
        println!("   [WARNING] Attempting to mint tokens - this may fail if User 2 is not the mint authority");
        // Calculate cost with fractional price support: cost = (price * quantity) / PRICE_SCALE
        // For price = 1.0 (1_000_000_000) and quantity = 2_000_000_000:
        // cost = (1_000_000_000 * 2_000_000_000) / 1_000_000_000 = 2_000_000_000
        let price_scale = 1_000_000_000u64;
        let estimated_cost = ((no_orders[0].price as u128)
            .checked_mul(no_orders[0].quantity as u128)
            .unwrap()
            .checked_div(price_scale as u128)
            .unwrap()) as u64;
        mint_tokens(&program, payer, base_mint, user_collateral, estimated_cost);
        println!("   [OK] User 2 collateral account: {}", user_collateral);
        println!("   [OK] Minted {} collateral tokens (2 SOL)", estimated_cost);
    }

    // Step 4: Create NO token ATA to receive purchased NO tokens
    println!("\nStep 4: Creating NO token account...");
    let user_no = create_ata(&program, payer, payer.pubkey(), no_mint);
    println!("   [OK] User 2 NO token account: {}", user_no);

    // Step 5: Execute market buy for NO tokens
    println!("\nStep 5: Executing market buy for NO tokens...");
    // User 2 buys all available NO tokens (2 SOL worth)
    let buy_quantity = no_orders[0].quantity; // Buy all NO tokens from User 1
    println!("   Buying {} NO tokens (2 SOL worth)", buy_quantity);
    
    program
        .request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: payer.pubkey(),
            buyer_collateral_ata: user_collateral,
            buyer_receive_token_ata: user_no,
            market,
            order_book,
            yes_vault,
            no_vault,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MarketBuy {
            params: nfl_blockchain::MarketBuyParams {
                quantity: buy_quantity,
                want_yes: false, // Buying NO tokens
            },
        })
        .accounts(vec![AccountMeta::new(user1_collateral, false)])
        .send()
        .unwrap();
    println!("   [OK] Market buy executed: bought {} NO tokens", buy_quantity);

    // Verify order was filled
    let final_ob: nfl_blockchain::OrderBook = program.account(order_book).unwrap();
    println!("\n   Order book now has {} order(s)", final_ob.orders.len());

    // Check final balances
    let no_acc: TokenAccount = program.account(user_no).unwrap();
    let collateral_acc: TokenAccount = program.account(user_collateral).unwrap();

    println!("\n==========================================");
    println!("User 2 Final Position:");
    println!("==========================================");
    println!("NO tokens: {} (purchased, 2 SOL worth)", no_acc.amount);
    println!("Collateral: {} (spent 2 SOL on purchase)", collateral_acc.amount);
    println!("\nDemo 3 completed successfully!");
    println!("User 2 has successfully purchased NO tokens from User 1!");
    println!("\nExpected outcome if YES wins:");
    println!("  - User 2 holds NO tokens: {} (worthless if YES wins)", no_acc.amount);
    println!("  - User 2 total: 0 SOL (-2 SOL loss)");
    println!("\nExpected outcome if NO wins:");
    println!("  - User 2 redeems NO tokens: {} (2 SOL)", no_acc.amount);
    println!("  - User 2 total: 2 SOL (break even, paid 2 SOL)");
}

fn get_env_pubkey(name: &str) -> Pubkey {
    std::env::var(name)
        .unwrap_or_else(|_| {
            eprintln!("Error: {} environment variable is not set.", name);
            eprintln!("Please run demo1 first and export the market information.");
            std::process::exit(1);
        })
        .parse()
        .unwrap_or_else(|_| {
            eprintln!("Error: Invalid {} address.", name);
            std::process::exit(1);
        })
}

