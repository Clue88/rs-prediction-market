//! Demo 2: User 1 (Buyer of YES) bets YES on the outcome
//! 
//! This script demonstrates:
//! 1. User 1 mints YES/NO token pairs (deposits collateral)
//! 2. User 1 sells NO tokens to isolate YES position (betting YES)
//!
//! Usage:
//!   export ANCHOR_WALLET=~/.config/solana/user1.json  # User 1's wallet
//!   export MARKET=<market_address>  # From demo1 output
//!   export BASE_MINT=<base_mint_address>  # From demo1 output
//!   export YES_MINT=<yes_mint_address>  # From demo1 output
//!   export NO_MINT=<no_mint_address>  # From demo1 output
//!   export VAULT=<vault_address>  # From demo1 output
//!   export MARKET_AUTHORITY=<market_authority_address>  # From demo1 output
//!   export ORDER_BOOK=<order_book_address>  # From demo1 output
//!   export YES_VAULT=<yes_vault_address>  # From demo1 output
//!   export NO_VAULT=<no_vault_address>  # From demo1 output
//!   cargo run --bin demo2

#![allow(deprecated)]

use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use std::str::FromStr;

use tests::test_utils::*;

fn main() {
    println!("==========================================");
    println!("Demo 2: User 1 (Buyer of YES)");
    println!("Betting YES on the outcome...");
    println!("==========================================\n");

    // Check for required environment variables
    let market = get_env_pubkey("MARKET");
    let base_mint = get_env_pubkey("BASE_MINT");
    let yes_mint = get_env_pubkey("YES_MINT");
    let no_mint = get_env_pubkey("NO_MINT");
    let vault = get_env_pubkey("VAULT");
    let market_authority = get_env_pubkey("MARKET_AUTHORITY");
    let order_book = get_env_pubkey("ORDER_BOOK");
    let yes_vault = get_env_pubkey("YES_VAULT");
    let no_vault = get_env_pubkey("NO_VAULT");

    // Setup client as User 1
    println!("Step 1: Setting up client as User 1...");
    let (program, payer) = setup_client();
    println!("   [OK] User 1 address: {}", payer.pubkey());

    // Step 2: Create collateral account and fund it
    println!("\nStep 2: Creating and funding collateral account...");
    let user_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    
    // Check if this is wSOL (So11111111111111111111111111111111111111112)
    let wsol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let is_wsol = base_mint == wsol_mint;
    
    if is_wsol {
        println!("   [OK] Using wSOL as collateral (no minting needed)");
        println!("   [OK] User 1 collateral account: {}", user_collateral);
        println!("   [INFO] User 1 should already have wSOL from wrapping");
    } else {
        // For non-wSOL mints, need to mint tokens
        // Note: This requires the mint authority, which User 1 may not have
        // In a real scenario, Market Authority would mint tokens for User 1, or User 1 would receive them via transfer
        println!("   [WARNING] Attempting to mint tokens - this may fail if User 1 is not the mint authority");
        mint_tokens(&program, payer, base_mint, user_collateral, 1000);
        println!("   [OK] User 1 collateral account: {}", user_collateral);
        println!("   [OK] Minted 1000 collateral tokens");
    }

    // Step 3: Mint YES/NO pairs (deposit collateral, receive outcome tokens)
    println!("\nStep 3: Minting YES/NO token pairs...");
    // User 1 wagers 2 SOL = 2_000_000_000 lamports (SOL has 9 decimals)
    let wager_amount = 4_000_000_000; // 2 SOL
    let (user_yes, user_no) = mint_pairs_for_user(
        &program,
        market,
        base_mint,
        yes_mint,
        no_mint,
        vault,
        market_authority,
        payer,
        user_collateral,
        wager_amount,
    );
    println!("   [OK] Minted {} YES/NO pairs (2 SOL wagered)", wager_amount);
    println!("   [OK] User 1 YES tokens: {}", user_yes);
    println!("   [OK] User 1 NO tokens: {}", user_no);

    // Step 4: Place limit sell order for NO tokens (to isolate YES position)
    println!("\nStep 4: Placing limit sell order for NO tokens...");
    println!("   (Selling NO tokens to bet on YES)");
    let sell_price = 500_000_000; // Price = 0.5 (scaled by PRICE_SCALE)
    let sell_quantity = wager_amount; // Selling all NO tokens (2 SOL worth)

    program
        .request()
        .accounts(nfl_blockchain::accounts::PlaceLimitSell {
            seller: payer.pubkey(),
            seller_token_ata: user_no,
            seller_receive_collateral_ata: user_collateral,
            order_book,
            yes_vault,
            no_vault,
            market,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::PlaceLimitSell {
            price: sell_price,
            quantity: sell_quantity,
            is_yes: false, // Selling NO tokens
        })
        .send()
        .unwrap();
    println!("   [OK] Placed sell order: {} NO tokens at price {}", sell_quantity, sell_price);

    // Verify order was placed
    let ob_account: nfl_blockchain::OrderBook = program.account(order_book).unwrap();
    println!("\n   Order book now has {} order(s)", ob_account.orders.len());
    if let Some(order) = ob_account.orders.first() {
        println!("   Order details: ID={}, Price={}, Quantity={}, IsYes={}", 
                 order.id, order.price, order.quantity, order.is_yes);
    }

    // Check final balances
    use anchor_spl::token::TokenAccount;
    let yes_acc: TokenAccount = program.account(user_yes).unwrap();
    let no_acc: TokenAccount = program.account(user_no).unwrap();
    let collateral_acc: TokenAccount = program.account(user_collateral).unwrap();

    println!("\n==========================================");
    println!("User 1 Final Position:");
    println!("==========================================");
    println!("YES tokens: {} (betting YES)", yes_acc.amount);
    println!("NO tokens: {} (sold)", no_acc.amount);
    println!("Collateral: {} (received from selling NO tokens)", collateral_acc.amount);
    println!("\nDemo 2 completed successfully!");
    println!("User 1 has successfully bet YES on the outcome!");
    println!("\nExpected outcome if YES wins:");
    println!("  - User 1 redeems YES tokens: {} (2 SOL)", yes_acc.amount);
    println!("  - User 1 already received: {} (2 SOL from selling NO)", collateral_acc.amount);
    println!("  - Total: 4 SOL (+2 SOL profit)");
    
    println!("\nTo use in demo3, export this as an environment variable:");
    println!("export USER1_COLLATERAL={}", user_collateral);
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
