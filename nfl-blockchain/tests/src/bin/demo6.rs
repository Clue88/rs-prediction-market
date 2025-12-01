//! Demo 6: User 2 attempts to redeem NO tokens after market resolution
//! 
//! This script demonstrates:
//! 1. User 2 attempts to redeem NO tokens after market is resolved to YES
//! 2. Redemption fails because User 2 holds losing NO tokens (market resolved to YES)
//!
//! Usage:
//!   export ANCHOR_WALLET=~/.config/solana/user2.json  # User 2's wallet
//!   export MARKET=<market_address>  # From demo1 output
//!   export BASE_MINT=<base_mint_address>  # From demo1 output
//!   export YES_MINT=<yes_mint_address>  # From demo1 output
//!   export NO_MINT=<no_mint_address>  # From demo1 output
//!   export VAULT=<vault_address>  # From demo1 output
//!   export MARKET_AUTHORITY=<market_authority_address>  # From demo1 output
//!   cargo run --bin demo6

#![allow(deprecated)]

use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use anchor_spl::token::TokenAccount;

use tests::test_utils::*;

fn main() {
    println!("==========================================");
    println!("Demo 6: User 2");
    println!("Attempting to redeem NO tokens (should fail)...");
    println!("==========================================\n");

    // Check for required environment variables
    let market = get_env_pubkey("MARKET");
    let base_mint = get_env_pubkey("BASE_MINT");
    let yes_mint = get_env_pubkey("YES_MINT");
    let no_mint = get_env_pubkey("NO_MINT");
    let vault = get_env_pubkey("VAULT");
    let market_authority = get_env_pubkey("MARKET_AUTHORITY");

    // Setup client as User 2
    println!("Step 1: Setting up client as User 2...");
    let (program, payer) = setup_client();
    println!("   [OK] User 2 address: {}", payer.pubkey());

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
        eprintln!("   [WARNING] Market outcome is not YES!");
        eprintln!("   This demo expects market to be resolved to YES.");
        eprintln!("   If market resolved to NO, User 2 would be able to redeem NO tokens.");
    }

    println!("   [OK] Market is resolved to YES");
    println!("   [INFO] User 2 holds NO tokens, which are losing tokens in this scenario");

    // Step 3: Get User 2's token accounts
    println!("\nStep 3: Getting User 2's token accounts...");
    let user_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    let user_yes = create_ata(&program, payer, payer.pubkey(), yes_mint);
    let user_no = create_ata(&program, payer, payer.pubkey(), no_mint);
    
    println!("   [OK] User 2 collateral account: {}", user_collateral);
    println!("   [OK] User 2 YES token account: {}", user_yes);
    println!("   [OK] User 2 NO token account: {}", user_no);

    // Step 4: Check balances before redemption attempt
    println!("\nStep 4: Checking balances before redemption attempt...");
    let yes_before: TokenAccount = program.account(user_yes).unwrap();
    let no_before: TokenAccount = program.account(user_no).unwrap();
    let collateral_before: TokenAccount = program.account(user_collateral).unwrap();
    
    println!("   YES tokens: {}", yes_before.amount);
    println!("   NO tokens: {}", no_before.amount);
    println!("   Collateral: {}", collateral_before.amount);

    if no_before.amount == 0 {
        eprintln!("   [ERROR] User 2 has no NO tokens!");
        eprintln!("   Please ensure User 2 has NO tokens (run demo3 first).");
        std::process::exit(1);
    }

    println!("   [INFO] User 2 has {} NO tokens (losing tokens)", no_before.amount);
    println!("   [INFO] User 2 has {} YES tokens (winning tokens, but User 2 doesn't hold any)", yes_before.amount);

    // Step 5: Attempt to redeem NO tokens (should fail)
    println!("\nStep 5: Attempting to redeem NO tokens...");
    println!("   (This should fail because market resolved to YES, not NO)");
    
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
            eprintln!("\n   [ERROR] Redemption unexpectedly succeeded!");
            eprintln!("   User 2 should not be able to redeem NO tokens when market resolved to YES.");
            std::process::exit(1);
        }
        Err(e) => {
            println!("   [OK] Redemption failed as expected!");
            println!("   Error: {:?}", e);
            
            // Check balances after failed redemption (should be unchanged)
            println!("\nStep 6: Checking balances after failed redemption...");
            let yes_after: TokenAccount = program.account(user_yes).unwrap();
            let no_after: TokenAccount = program.account(user_no).unwrap();
            let collateral_after: TokenAccount = program.account(user_collateral).unwrap();
            
            println!("   YES tokens: {} (unchanged)", yes_after.amount);
            println!("   NO tokens: {} (unchanged)", no_after.amount);
            println!("   Collateral: {} (unchanged)", collateral_after.amount);
            
            // Verify balances are unchanged
            if yes_before.amount != yes_after.amount {
                eprintln!("   [WARNING] YES tokens changed unexpectedly!");
            }
            if no_before.amount != no_after.amount {
                eprintln!("   [WARNING] NO tokens changed unexpectedly!");
            }
            if collateral_before.amount != collateral_after.amount {
                eprintln!("   [WARNING] Collateral changed unexpectedly!");
            }
            
            println!("\n==========================================");
            println!("Redemption Summary:");
            println!("==========================================");
            println!("Status: FAILED (as expected)");
            println!("Reason: User 2 holds NO tokens, but market resolved to YES");
            println!("Result: NO tokens cannot be redeemed when market outcome is YES");
            println!("\nDemo 6 completed successfully!");
            println!("User 2 correctly cannot redeem losing NO tokens!");
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

