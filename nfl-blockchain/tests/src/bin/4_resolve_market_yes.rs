//! Demo 4: Market Authority resolves the market to YES
//! 
//! This script demonstrates:
//! 1. Market Authority resolves the market to YES
//! 2. Verifies the market resolution
//!
//! Usage:
//!   export ANCHOR_WALLET=~/.config/solana/marketauth.json  # Market Authority's wallet
//!   export MARKET=<market_address>  # From demo1 output
//!   cargo run --bin demo4

#![allow(deprecated)]

use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Signer;

use tests::test_utils::*;

fn main() {
    println!("Market Authority resolving market to YES...\n");

    // Check for required environment variables
    let market = get_env_pubkey("MARKET");

    // Setup client as Market Authority
    println!("Step 1: Setting up client as Market Authority...");
    let (program, payer) = setup_client();
    println!("   [OK] Market Authority address: {}", payer.pubkey());

    // Check market status before resolution
    println!("\nStep 2: Checking market status before resolution...");
    let market_before: nfl_blockchain::Market = program.account(market).unwrap();
    println!("   Market status: {:?}", market_before.status);
    println!("   Market outcome: {:?}", market_before.outcome);

    // Step 3: Resolve the market to YES
    println!("\nStep 3: Resolving market to YES...");
    let resolution_outcome = nfl_blockchain::Outcome::Yes;
    
    program
        .request()
        .accounts(nfl_blockchain::accounts::ResolveMarket {
            authority: payer.pubkey(),
            market,
        })
        .args(nfl_blockchain::instruction::ResolveMarket {
            outcome: resolution_outcome,
        })
        .send()
        .unwrap();
    
    println!("   [OK] Market resolution transaction sent");

    // Verify market was resolved
    println!("\nStep 4: Verifying market resolution...");
    let resolved_market: nfl_blockchain::Market = program.account(market).unwrap();
    println!("   Market status: {:?}", resolved_market.status);
    println!("   Market outcome: {:?}", resolved_market.outcome);

    if resolved_market.outcome == nfl_blockchain::Outcome::Yes {
        println!("\n   [OK] Market successfully resolved to YES!");
    } else {
        eprintln!("\n   [ERROR] Market was not resolved to YES as expected!");
        std::process::exit(1);
    }


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

