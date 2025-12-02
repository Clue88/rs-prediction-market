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
    println!("User 2 Buying NO tokens from User 1...\n");


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

    // Step 2: Check order book for available NO token sell orders
    println!("\nStep 2: Checking order book for NO token sell orders...");
    let ob_account: nfl_blockchain::OrderBook = program.account(order_book).unwrap();
    
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

    println!("\n-------------");
    println!("User 2 Final Position:");
    println!("NO tokens: {} (purchased, 2 SOL worth)", no_acc.amount);
    println!("Collateral: {} (spent 2 SOL on purchase)", collateral_acc.amount);
    
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

