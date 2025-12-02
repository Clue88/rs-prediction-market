//! Minimal demo showcasing NFL Blockchain prediction market functionality
//! 
//! This demo demonstrates:
//! 1. Creating a prediction market
//! 2. Minting YES/NO token pairs
//! 3. Setting up an order book
//! 4. Placing limit sell orders
//! 5. Executing market buy orders

#![allow(deprecated)]

use anchor_client::solana_sdk::{
    instruction::AccountMeta, pubkey::Pubkey, signer::Signer,
};
use anchor_spl::token::TokenAccount;

use crate::test_utils::*;

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

pub fn run_demo() {
    println!("NFL Blockchain Demo - Starting...\n");

    // Check for required environment variable
    if std::env::var("ANCHOR_WALLET").is_err() {
        eprintln!("Error: ANCHOR_WALLET environment variable is not set.");
        eprintln!("Please set it to the path of your Solana keypair file.");
        eprintln!("Example: export ANCHOR_WALLET=~/.config/solana/id.json");
        std::process::exit(1);
    }

    // Step 1: Setup client and create base collateral mint
    println!("Step 1: Setting up client and creating base mint...");
    let (program, payer) = setup_client();
    let base_mint = create_mint(&program, payer).pubkey();
    println!("   [OK] Base mint created: {}\n", base_mint);

    // Step 2: Create a prediction market
    println!("Step 2: Creating prediction market...");
    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) =
        create_market(&program, payer, base_mint);
    println!("   [OK] Market created: {}", market_kp.pubkey());
    println!("   [OK] YES mint: {}", yes_mint_kp.pubkey());
    println!("   [OK] NO mint: {}\n", no_mint_kp.pubkey());

    // Step 3: User mints YES/NO pairs (deposits collateral, receives outcome tokens)
    println!("Step 3: User mints YES/NO token pairs...");
    let user_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    mint_tokens(&program, payer, base_mint, user_collateral, 1000);

    let (user_yes, user_no) = mint_pairs_for_user(
        &program,
        market_kp.pubkey(),
        base_mint,
        yes_mint_kp.pubkey(),
        no_mint_kp.pubkey(),
        vault_kp.pubkey(),
        market_authority,
        payer,
        user_collateral,
        100, // Mint 100 pairs
    );
    println!("   [OK] Minted 100 YES/NO pairs");
    println!("   [OK] User YES tokens: {}", user_yes);
    println!("   [OK] User NO tokens: {}\n", user_no);

    // Step 4: Initialize order book
    println!("Step 4: Initializing order book...");
    let order_book_pda = get_orderbook_pda(market_kp.pubkey());
    let yes_vault_pda = get_ob_vault_pda(order_book_pda, true);
    let no_vault_pda = get_ob_vault_pda(order_book_pda, false);

    program
        .request()
        .accounts(nfl_blockchain::accounts::InitializeOrderBook {
            authority: payer.try_pubkey().unwrap(),
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
    println!("   [OK] Order book initialized: {}\n", order_book_pda);

    // Step 5: Place a limit sell order
    println!("Step 5: Placing limit sell order...");
    let sell_price = 60; // Price per token (in collateral units)
    let sell_quantity = 20; // Selling 20 YES tokens

    program
        .request()
        .accounts(nfl_blockchain::accounts::PlaceLimitSell {
            seller: payer.try_pubkey().unwrap(),
            seller_token_ata: user_yes,
            seller_receive_collateral_ata: user_collateral,
            order_book: order_book_pda,
            yes_vault: yes_vault_pda,
            no_vault: no_vault_pda,
            market: market_kp.pubkey(),
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::PlaceLimitSell {
            price: sell_price,
            quantity: sell_quantity,
            is_yes: true,
        })
        .send()
        .unwrap();
    println!("   [OK] Placed sell order: {} YES tokens at price {}\n", sell_quantity, sell_price);

    // Verify order was placed
    let ob_account: nfl_blockchain::OrderBook = program.account(order_book_pda).unwrap();
    println!("   Order book now has {} order(s)", ob_account.orders.len());
    if let Some(order) = ob_account.orders.first() {
        println!("   Order details: ID={}, Price={}, Quantity={}, IsYes={}\n", 
                 order.id, order.price, order.quantity, order.is_yes);
    }

    // Step 6: Execute market buy
    println!("Step 6: Executing market buy...");
    let buyer_collateral = user_collateral; // Same user for demo simplicity
    let buyer_yes = user_yes; // Buyer will receive YES tokens here
    mint_tokens(&program, payer, base_mint, buyer_collateral, 2000); // Add more collateral

    let buy_quantity = 10; // Buy 10 YES tokens

    program
        .request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: payer.try_pubkey().unwrap(),
            buyer_collateral_ata: buyer_collateral,
            buyer_receive_token_ata: buyer_yes,
            market: market_kp.pubkey(),
            order_book: order_book_pda,
            yes_vault: yes_vault_pda,
            no_vault: no_vault_pda,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MarketBuy {
            params: nfl_blockchain::MarketBuyParams {
                quantity: buy_quantity,
                want_yes: true,
            },
        })
        .accounts(vec![AccountMeta::new(user_collateral, false)])
        .send()
        .unwrap();
    println!("   [OK] Market buy executed: bought {} YES tokens\n", buy_quantity);

    // Step 7: Verify final state
    println!("Step 7: Verifying final state...");
    let buyer_yes_acc: TokenAccount = program.account(buyer_yes).unwrap();
    let buyer_collateral_acc: TokenAccount = program.account(buyer_collateral).unwrap();
    let final_ob: nfl_blockchain::OrderBook = program.account(order_book_pda).unwrap();

    println!("   [OK] Buyer YES tokens: {}", buyer_yes_acc.amount);
    println!("   [OK] Buyer collateral: {}", buyer_collateral_acc.amount);
    println!("   [OK] Remaining orders in book: {}", final_ob.orders.len());
    
    if let Some(order) = final_ob.orders.first() {
        println!("   [OK] Remaining order quantity: {}", order.quantity);
    }

    println!("\nDemo completed successfully!");
}

