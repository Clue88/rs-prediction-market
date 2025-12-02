//! Demo 1: Market Authority creates a market
//! 
//! This script demonstrates:
//! 1. Setting up the client as Market Authority
//! 2. Creating a base collateral mint
//! 3. Creating a prediction market
//! 4. Initializing the order book
//!
//! Usage:
//!   export ANCHOR_WALLET=~/.config/solana/marketauth.json  # Market Authority's wallet
//!   cargo run --bin demo1

#![allow(deprecated)]

use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use std::str::FromStr;
use tests::test_utils::*;

fn get_orderbook_pda(market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"orderbook", market.as_ref()],
        &nfl_blockchain::id(),
    )
    .0
}

fn get_ob_vault_pda(order_book: Pubkey, is_yes: bool) -> Pubkey {
    let seed: &[u8] = if is_yes { b"yes_vault" } else { b"no_vault" };
    Pubkey::find_program_address(&[seed, order_book.as_ref()], &nfl_blockchain::id()).0
}

fn main() {
    println!("Market Authority");
    println!("Creating a prediction market...");
    println!("==========================================\n");

    // Check for required environment variable
    if std::env::var("ANCHOR_WALLET").is_err() {
        eprintln!("Error: ANCHOR_WALLET environment variable is not set.");
        eprintln!("Please set it to the path of Market Authority's Solana keypair file.");
        eprintln!("Example: export ANCHOR_WALLET=~/.config/solana/marketauth.json");
        std::process::exit(1);
    }

    // Step 1: Setup client and create base collateral mint
    println!("Step 1: Setting up market authority as client and creating base mint...");
    let (program, payer) = setup_client();
    let base_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();    println!("   [OK] Base mint created: {}", base_mint);

    // Step 2: Create a prediction market
    println!("\nStep 2: Creating prediction market...");
    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) =
        create_market(&program, payer, base_mint);


    // Step 3: Initialize order book
    println!("\nStep 3: Initializing order book...");
    let order_book_pda = get_orderbook_pda(market_kp.pubkey());
    let yes_vault_pda = get_ob_vault_pda(order_book_pda, true);
    let no_vault_pda = get_ob_vault_pda(order_book_pda, false);

    program
        .request()
        .accounts(nfl_blockchain::accounts::InitializeOrderBook {
            authority: payer.pubkey(),
            order_book: order_book_pda,
            market: market_kp.pubkey(),
            yes_mint: yes_mint_kp.pubkey(),
            no_mint: no_mint_kp.pubkey(),
            yes_vault: yes_vault_pda,
            no_vault: no_vault_pda,
            token_program: anchor_spl::token::spl_token::id(),
            system_program: anchor_client::solana_sdk::system_program::id(),
            rent: anchor_client::solana_sdk::sysvar::rent::id(),
        })
        .args(nfl_blockchain::instruction::InitializeOrderBook {})
        .send()
        .unwrap();
    println!("   [OK] Order book initialized: {}", order_book_pda);

    // Print market information for use in subsequent demos

    println!("\nTo market info in next demos, export these as environment variables:");
    println!("export MARKET={}", market_kp.pubkey());
    println!("export BASE_MINT={}", base_mint);
    println!("export YES_MINT={}", yes_mint_kp.pubkey());
    println!("export NO_MINT={}", no_mint_kp.pubkey());
    println!("export VAULT={}", vault_kp.pubkey());
    println!("export MARKET_AUTHORITY={}", market_authority);
    println!("export ORDER_BOOK={}", order_book_pda);
    println!("export YES_VAULT={}", yes_vault_pda);
    println!("export NO_VAULT={}", no_vault_pda);
}

