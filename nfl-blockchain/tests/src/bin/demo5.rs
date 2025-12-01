//! Demo 5: User 1 redeems YES tokens after market resolution
//! 
//! This script demonstrates:
//! 1. User 1 attempts to redeem YES tokens after market is resolved to YES
//! 2. Redemption succeeds because User 1 holds winning YES tokens
//!
//! Usage:
//!   export ANCHOR_WALLET=~/.config/solana/user1.json  # User 1's wallet
//!   export MARKET=<market_address>  # From demo1 output
//!   export BASE_MINT=<base_mint_address>  # From demo1 output
//!   export YES_MINT=<yes_mint_address>  # From demo1 output
//!   export NO_MINT=<no_mint_address>  # From demo1 output
//!   export VAULT=<vault_address>  # From demo1 output
//!   export MARKET_AUTHORITY=<market_authority_address>  # From demo1 output
//!   cargo run --bin demo5

#![allow(deprecated)]

use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use anchor_spl::token::TokenAccount;

use tests::test_utils::*;

fn main() {
    println!("==========================================");
    println!("Demo 5: User 1");
    println!("Redeeming YES tokens (should succeed)...");
    println!("==========================================\n");

    // Check for required environment variables
    let market = get_env_pubkey("MARKET");
    let base_mint = get_env_pubkey("BASE_MINT");
    let yes_mint = get_env_pubkey("YES_MINT");
    let no_mint = get_env_pubkey("NO_MINT");
    let vault = get_env_pubkey("VAULT");
    let market_authority = get_env_pubkey("MARKET_AUTHORITY");

    // Setup client as User 1
    println!("Step 1: Setting up client as User 1...");
    let (program, payer) = setup_client();
    println!("   [OK] User 1 address: {}", payer.pubkey());

    // Step 2: Verify market is resolved
    println!("\nStep 2: Checking market status...");
    let market_account: nfl_blockchain::Market = program.account(market).unwrap();
    println!("   Market status: {:?}", market_account.status);
    println!("   Market outcome: {:?}", market_account.outcome);

    if market_account.status != nfl_blockchain::MarketStatus::Resolved {
        eprintln!("   [ERROR] Market is not resolved yet!");
        eprintln!("   Please run demo4 first to resolve the market.");
        std::process::exit(1);
    }

    if market_account.outcome != nfl_blockchain::Outcome::Yes {
        eprintln!("   [ERROR] Market outcome is not YES!");
        eprintln!("   User 1 can only redeem YES tokens if market resolved to YES.");
        std::process::exit(1);
    }

    println!("   [OK] Market is resolved to YES");

    // Step 3: Get User 1's token accounts
    println!("\nStep 3: Getting User 1's token accounts...");
    let user_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    let user_yes = create_ata(&program, payer, payer.pubkey(), yes_mint);
    let user_no = create_ata(&program, payer, payer.pubkey(), no_mint);
    
    println!("   [OK] User 1 collateral account: {}", user_collateral);
    println!("   [OK] User 1 YES token account: {}", user_yes);
    println!("   [OK] User 1 NO token account: {}", user_no);

    // Step 4: Check balances before redemption
    println!("\nStep 4: Checking balances before redemption...");
    let yes_before: TokenAccount = program.account(user_yes).unwrap();
    let no_before: TokenAccount = program.account(user_no).unwrap();
    let collateral_before: TokenAccount = program.account(user_collateral).unwrap();
    
    println!("   YES tokens: {}", yes_before.amount);
    println!("   NO tokens: {}", no_before.amount);
    println!("   Collateral: {}", collateral_before.amount);

    if yes_before.amount == 0 {
        eprintln!("   [ERROR] User 1 has no YES tokens to redeem!");
        eprintln!("   Please ensure User 1 has YES tokens (run demo2 first).");
        std::process::exit(1);
    }

    // Step 5: Attempt to redeem YES tokens
    println!("\nStep 5: Attempting to redeem YES tokens...");
    
    let result = program
        .request()
        .accounts(nfl_blockchain::accounts::Redeem {
            user: payer.pubkey(),
            market,
            base_mint,
            yes_mint,
            no_mint,
            vault,
            user_collateral_ata: user_collateral,
            user_yes_ata: user_yes,
            user_no_ata: user_no,
            market_authority,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::Redeem {})
        .send();

    match result {
        Ok(_) => {
            println!("   [OK] Redemption transaction sent successfully!");
            
            // Step 6: Check balances after redemption
            println!("\nStep 6: Checking balances after redemption...");
            let yes_after: TokenAccount = program.account(user_yes).unwrap();
            let collateral_after: TokenAccount = program.account(user_collateral).unwrap();
            
            println!("   YES tokens: {} (burned)", yes_after.amount);
            println!("   Collateral: {} (received {} collateral)", 
                     collateral_after.amount, 
                     collateral_after.amount.saturating_sub(collateral_before.amount));
            
            println!("\n==========================================");
            println!("Redemption Summary:");
            println!("==========================================");
            println!("Status: SUCCESS");
            println!("User 1 successfully redeemed {} YES tokens", yes_before.amount);
            println!("Received collateral: {}", 
                     collateral_after.amount.saturating_sub(collateral_before.amount));
            println!("\nDemo 5 completed successfully!");
            println!("User 1 has successfully redeemed their winning YES tokens!");
        }
        Err(e) => {
            eprintln!("   [ERROR] Redemption failed: {:?}", e);
            eprintln!("   This should not happen - User 1 has YES tokens and market is resolved to YES.");
            std::process::exit(1);
        }
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

