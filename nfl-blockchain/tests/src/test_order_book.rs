#![allow(deprecated)]

use anchor_client::solana_sdk::{
    instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
};
use anchor_spl::token::TokenAccount;

use crate::test_utils::*;

// --- Helpers ---
fn get_orderbook_pda(market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"orderbook", market.as_ref()],
        &nfl_blockchain::id(),
    ).0
}

fn get_ob_vault_pda(order_book: Pubkey, is_yes: bool) -> Pubkey {
    // Cast to slice to match types
    let seed: &[u8] = if is_yes { b"yes_vault" } else { b"no_vault" };
    Pubkey::find_program_address(
        &[seed, order_book.as_ref()],
        &nfl_blockchain::id(),
    ).0
}

fn fund_account(program: &anchor_client::Program<&Keypair>, payer: &Keypair, to: &Pubkey, amount: u64) {
    program.request()
        .instruction(system_instruction::transfer(&payer.pubkey(), to, amount))
        .send()
        .unwrap();
}

// --- Tests ---

#[test]
fn test_01_init_order_book() {
    // Setup the client and create the required Market and mints before attaching an Order Book
    let (program, payer) = setup_client();
    let base_mint = create_mint(&program, payer).pubkey();
    let (market_kp, yes_mint_kp, no_mint_kp, _, _) = create_market(&program, payer, base_mint);

    // Derive the deterministic addresses (PDAs) where the Order Book and its vaults will live
    let order_book_pda = get_orderbook_pda(market_kp.pubkey());
    let yes_vault_pda = get_ob_vault_pda(order_book_pda, true);
    let no_vault_pda = get_ob_vault_pda(order_book_pda, false);

    // Send the transaction to initialize the Order Book and create the token vaults on-chain
    program.request()
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

    // Fetch the newly created account to verify it exists with the correct default state
    let ob_account: nfl_blockchain::OrderBook = program.account(order_book_pda).unwrap();
    assert_eq!(ob_account.capacity, 100);
}

#[test]
fn test_02_place_limit_sell() {
    // Initialize the market and order book infrastructure required for trading
    let (program, payer) = setup_client();
    let base_mint = create_mint(&program, payer).pubkey();
    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) = create_market(&program, payer, base_mint);

    let order_book_pda = get_orderbook_pda(market_kp.pubkey());
    let yes_vault_pda = get_ob_vault_pda(order_book_pda, true);
    let no_vault_pda = get_ob_vault_pda(order_book_pda, false);

    program.request().accounts(nfl_blockchain::accounts::InitializeOrderBook {
        authority: payer.pubkey(), order_book: order_book_pda, market: market_kp.pubkey(),
        yes_mint: yes_mint_kp.pubkey(), no_mint: no_mint_kp.pubkey(),
        yes_vault: yes_vault_pda, no_vault: no_vault_pda,
        token_program: anchor_spl::token::spl_token::id(), system_program: anchor_client::solana_sdk::system_program::id(), rent: anchor_client::solana_sdk::sysvar::rent::id(),
    }).args(nfl_blockchain::instruction::InitializeOrderBook {}).send().unwrap();

    // Fund the user with collateral and mint YES/NO pairs to acquire the specific outcome tokens needed to place a sell order
    let user_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    mint_tokens(&program, payer, base_mint, user_collateral, 100);
    
    let (user_yes, _) = mint_pairs_for_user(
        &program, market_kp.pubkey(), base_mint, yes_mint_kp.pubkey(), no_mint_kp.pubkey(),
        vault_kp.pubkey(), market_authority, payer, user_collateral, 50
    );

    let price = 50;
    let quantity = 10;
    
    // Submit the Limit Sell transaction to escrow the tokens and list the order on the book
    program.request()
        .accounts(nfl_blockchain::accounts::PlaceLimitSell {
            seller: payer.pubkey(),
            seller_token_ata: user_yes,
            seller_receive_collateral_ata: user_collateral,
            order_book: order_book_pda,
            yes_vault: yes_vault_pda,
            no_vault: no_vault_pda,
            market: market_kp.pubkey(),
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::PlaceLimitSell { price, quantity, is_yes: true })
        .send()
        .unwrap();

    // Query the on-chain Order Book account to confirm the order was successfully recorded
    let ob_account: nfl_blockchain::OrderBook = program.account(order_book_pda).unwrap();
    assert_eq!(ob_account.orders.len(), 1);
    assert_eq!(ob_account.orders[0].price, 50);
}

#[test]
fn test_03_market_buy_full_fill() {
    // Initialize the market and order book infrastructure required for trading
    let (program, payer) = setup_client();
    let base_mint = create_mint(&program, payer).pubkey();
    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) = create_market(&program, payer, base_mint);

    let order_book_pda = get_orderbook_pda(market_kp.pubkey());
    let yes_vault_pda = get_ob_vault_pda(order_book_pda, true);
    let no_vault_pda = get_ob_vault_pda(order_book_pda, false);

    program.request().accounts(nfl_blockchain::accounts::InitializeOrderBook {
        authority: payer.pubkey(), order_book: order_book_pda, market: market_kp.pubkey(),
        yes_mint: yes_mint_kp.pubkey(), no_mint: no_mint_kp.pubkey(),
        yes_vault: yes_vault_pda, no_vault: no_vault_pda,
        token_program: anchor_spl::token::spl_token::id(), system_program: anchor_client::solana_sdk::system_program::id(), rent: anchor_client::solana_sdk::sysvar::rent::id(),
    }).args(nfl_blockchain::instruction::InitializeOrderBook {}).send().unwrap();

    // Create a distinct Seller identity and fund them with collateral to mint the YES tokens they intend to sell
    let seller_kp = Keypair::new();
    fund_account(&program, payer, &seller_kp.pubkey(), 1_000_000_000); 

    let seller_collateral = create_ata(&program, payer, seller_kp.pubkey(), base_mint);
    mint_tokens(&program, payer, base_mint, seller_collateral, 100);
    
    // Manually construct the mint transaction to ensure the new Seller keypair signs it correctly
    let seller_yes = create_ata(&program, payer, seller_kp.pubkey(), yes_mint_kp.pubkey());
    let seller_no = create_ata(&program, payer, seller_kp.pubkey(), no_mint_kp.pubkey());
    
    program.request()
        .accounts(nfl_blockchain::accounts::MintPairs {
            user: seller_kp.pubkey(),
            user_collateral_ata: seller_collateral,
            market: market_kp.pubkey(),
            base_mint,
            yes_mint: yes_mint_kp.pubkey(),
            no_mint: no_mint_kp.pubkey(),
            vault: vault_kp.pubkey(),
            user_yes_ata: seller_yes,
            user_no_ata: seller_no,
            market_authority,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MintPairs { amount: 20 })
        .signer(&seller_kp) // <--- THIS IS THE FIX (Sign as Seller)
        .send()
        .unwrap();

    // Seller places a Limit Sell order, escrowing their YES tokens into the program vault
    program.request()
        .accounts(nfl_blockchain::accounts::PlaceLimitSell {
            seller: seller_kp.pubkey(), seller_token_ata: seller_yes, seller_receive_collateral_ata: seller_collateral,
            order_book: order_book_pda, yes_vault: yes_vault_pda, no_vault: no_vault_pda, market: market_kp.pubkey(), token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::PlaceLimitSell { price: 50, quantity: 10, is_yes: true })
        .signer(&seller_kp)
        .send().unwrap();

    // Buyer (acting as the default Payer) executes a Market Buy to purchase 10 YES tokens at the best available price 
    let buyer_pubkey = payer.pubkey();
    let buyer_collateral = create_ata(&program, payer, buyer_pubkey, base_mint);
    let buyer_yes = create_ata(&program, payer, buyer_pubkey, yes_mint_kp.pubkey());
    mint_tokens(&program, payer, base_mint, buyer_collateral, 1000);

    println!("DEBUG: Sending Market Buy transaction...");
    program.request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: buyer_pubkey, 
            buyer_collateral_ata: buyer_collateral,
            buyer_receive_token_ata: buyer_yes,
            market: market_kp.pubkey(),
            order_book: order_book_pda,
            yes_vault: yes_vault_pda,
            no_vault: no_vault_pda,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MarketBuy {
            params: nfl_blockchain::MarketBuyParams { quantity: 10, want_yes: true }
        })
        .accounts(vec![ AccountMeta::new(seller_collateral, false) ]) 
        .signer(payer) // Explicit sign for payer just in case
        .send() 
        .unwrap();

    // Verify the atomic swap: Buyer received the outcome tokens, Seller received the USDC collateral, and the order was removed
    let buyer_yes_acc: TokenAccount = program.account(buyer_yes).unwrap();
    assert_eq!(buyer_yes_acc.amount, 10); 

    let seller_collateral_acc: TokenAccount = program.account(seller_collateral).unwrap();
    assert_eq!(seller_collateral_acc.amount, 580); 

    let ob_account: nfl_blockchain::OrderBook = program.account(order_book_pda).unwrap();
    assert_eq!(ob_account.orders.len(), 0);
}

#[test]
fn test_04_buy_exact_fail_too_expensive() {
    // Initialize the market and order book infrastructure required for trading
    let (program, payer) = setup_client();
    let base_mint = create_mint(&program, payer).pubkey();
    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) = create_market(&program, payer, base_mint);

    let order_book_pda = get_orderbook_pda(market_kp.pubkey());
    let yes_vault_pda = get_ob_vault_pda(order_book_pda, true);
    let no_vault_pda = get_ob_vault_pda(order_book_pda, false);

    program.request().accounts(nfl_blockchain::accounts::InitializeOrderBook {
        authority: payer.pubkey(), order_book: order_book_pda, market: market_kp.pubkey(),
        yes_mint: yes_mint_kp.pubkey(), no_mint: no_mint_kp.pubkey(),
        yes_vault: yes_vault_pda, no_vault: no_vault_pda,
        token_program: anchor_spl::token::spl_token::id(), system_program: anchor_client::solana_sdk::system_program::id(), rent: anchor_client::solana_sdk::sysvar::rent::id(),
    }).args(nfl_blockchain::instruction::InitializeOrderBook {}).send().unwrap();

    // Place a Limit Sell order at a price of 80 to establish liquidity on the book that is intentionally too expensive for the test case
    let seller_collateral = create_ata(&program, payer, payer.pubkey(), base_mint);
    mint_tokens(&program, payer, base_mint, seller_collateral, 100);
    
    let (seller_yes, _) = mint_pairs_for_user(
        &program, market_kp.pubkey(), base_mint, yes_mint_kp.pubkey(), no_mint_kp.pubkey(),
        vault_kp.pubkey(), market_authority, payer, seller_collateral, 20
    );

    program.request()
        .accounts(nfl_blockchain::accounts::PlaceLimitSell {
            seller: payer.pubkey(), seller_token_ata: seller_yes, seller_receive_collateral_ata: seller_collateral,
            order_book: order_book_pda, yes_vault: yes_vault_pda, no_vault: no_vault_pda, market: market_kp.pubkey(), token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::PlaceLimitSell { price: 80, quantity: 10, is_yes: true })
        .send().unwrap();

    // Since the Buyer and Seller are the same entity in this test, reuse the existing collateral and token accounts to avoid errors
    let buyer_pubkey = payer.pubkey();
    let buyer_collateral = seller_collateral; 
    let buyer_yes = seller_yes; 

    mint_tokens(&program, payer, base_mint, buyer_collateral, 1000);

    // Attempt to execute a Buy Exact order with a max price of 60, which should fail because the only available liquidity is priced at 80
    println!("DEBUG: Sending Buy Exact transaction...");
    let result = program.request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: buyer_pubkey, 
            buyer_collateral_ata: buyer_collateral, 
            buyer_receive_token_ata: buyer_yes,
            market: market_kp.pubkey(), 
            order_book: order_book_pda, 
            yes_vault: yes_vault_pda, 
            no_vault: no_vault_pda,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::BuyExact {
            params: nfl_blockchain::BuyExactParams { quantity: 10, want_yes: true, max_price: 60 }
        })
        .accounts(vec![AccountMeta::new(seller_collateral, false)])
        .signer(payer) 
        .send();

    // Assert that the transaction was rejected by the program as expected, enforcing the price protection logic
    assert!(result.is_err());
}