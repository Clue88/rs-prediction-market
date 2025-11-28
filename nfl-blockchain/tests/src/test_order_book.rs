use std::str::FromStr;
use std::ops::Deref; 

#[allow(deprecated)]
use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
        signature::Keypair, signer::Signer, system_instruction, system_program, sysvar::rent,
        instruction::AccountMeta,
    },
    Client, Cluster, Program,
};
use anchor_spl::token::{spl_token, Mint, TokenAccount};

fn print_balances<C: Deref<Target = impl Signer> + Clone>(
    program: &Program<C>,
    usdc: Pubkey,
    yes: Pubkey,
    owner: Pubkey,
    name: &str
) {
    let usdc_ata = spl_associated_token_account::get_associated_token_address(&owner, &usdc);
    let yes_ata = spl_associated_token_account::get_associated_token_address(&owner, &yes);
    // Handle cases where account might not exist yet (defaults to 0)
    let usdc_bal: u64 = if let Ok(acc) = program.account::<TokenAccount>(usdc_ata) { acc.amount } else { 0 };
    let yes_bal: u64 = if let Ok(acc) = program.account::<TokenAccount>(yes_ata) { acc.amount } else { 0 };

    println!("[{:<10}] USDC: {:<12} YES: {}", name, usdc_bal, yes_bal);
}

#[test]
fn test_order_book() {
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let payer = read_keypair_file(&anchor_wallet).unwrap();
    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program_id = Pubkey::from_str("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc").unwrap();
    let program = client.program(program_id).unwrap();

    let market = Keypair::new();
    let base_mint = Keypair::new();
    let yes_mint = Keypair::new();
    let no_mint = Keypair::new();
    let vault = Keypair::new();
    
    // --- DERIVE PDAS ---
    let (market_authority, _) = Pubkey::find_program_address(&[b"market_auth", market.pubkey().as_ref()], &program_id);
    let (order_book_pda, _) = Pubkey::find_program_address(&[b"orderbook", market.pubkey().as_ref()], &program_id);

    println!("--- Setting up Market Infrastructure ---");

    // 1. Init Base Mint
    let mint_rent = program.rpc().get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    program.request()
        .instruction(system_instruction::create_account(&payer.pubkey(), &base_mint.pubkey(), mint_rent, Mint::LEN as u64, &spl_token::id()))
        .instruction(spl_token::instruction::initialize_mint(&spl_token::id(), &base_mint.pubkey(), &payer.pubkey(), None, 6).unwrap())
        .signer(&base_mint)
        .send().expect("Failed to create base mint");

    // 2. Create Market
    program.request()
        .accounts(nfl_blockchain::accounts::CreateMarket {
            authority: payer.pubkey(),
            market: market.pubkey(),
            base_mint: base_mint.pubkey(),
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            market_authority,
            token_program: spl_token::id(),
            system_program: system_program::id(),
            rent: rent::id(),
        })
        .args(nfl_blockchain::instruction::CreateMarket { expiry_ts: 1_800_000_000 })
        .signer(&market)
        .signer(&yes_mint)
        .signer(&no_mint)
        .signer(&vault)
        .send().expect("Failed to create market");

    // 3. Create Vault ATAs for Market Authority
    let yes_vault_ata = spl_associated_token_account::get_associated_token_address(&market_authority, &yes_mint.pubkey());
    let no_vault_ata = spl_associated_token_account::get_associated_token_address(&market_authority, &no_mint.pubkey());
    
    program.request()
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &market_authority, &yes_mint.pubkey(), &spl_token::id()))
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &market_authority, &no_mint.pubkey(), &spl_token::id()))
        .send().expect("Failed to create YES/NO vaults");

    // 4. Initialize OrderBook PDA
    program.request()
        .accounts(nfl_blockchain::accounts::InitializeOrderBook {
            authority: payer.pubkey(),
            order_book: order_book_pda,
            market: market.pubkey(),
            system_program: system_program::id(),
        })
        .args(nfl_blockchain::instruction::InitializeOrderBook {})
        .send().expect("Failed to init orderbook");

    // --- Phase 1: Alice ---
    println!("\n--- Phase 1: Alice (Seller) Setup ---");
    let alice = Keypair::new();
    program.request().instruction(system_instruction::transfer(&payer.pubkey(), &alice.pubkey(), 1_000_000_000)).send().unwrap();

    let alice_usdc = spl_associated_token_account::get_associated_token_address(&alice.pubkey(), &base_mint.pubkey());
    let alice_yes = spl_associated_token_account::get_associated_token_address(&alice.pubkey(), &yes_mint.pubkey());
    let alice_no = spl_associated_token_account::get_associated_token_address(&alice.pubkey(), &no_mint.pubkey());

    program.request()
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &alice.pubkey(), &base_mint.pubkey(), &spl_token::id()))
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &alice.pubkey(), &yes_mint.pubkey(), &spl_token::id()))
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &alice.pubkey(), &no_mint.pubkey(), &spl_token::id()))
        .instruction(spl_token::instruction::mint_to(&spl_token::id(), &base_mint.pubkey(), &alice_usdc, &payer.pubkey(), &[], 100_000_000).unwrap())
        .send().unwrap();

    program.request()
        .accounts(nfl_blockchain::accounts::MintPairs {
            user: alice.pubkey(),
            user_collateral_ata: alice_usdc,
            market: market.pubkey(),
            base_mint: base_mint.pubkey(),
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            user_yes_ata: alice_yes,
            user_no_ata: alice_no,
            market_authority,
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MintPairs { amount: 100 })
        .signer(&alice)
        .send().unwrap();

    println!("Alice places Limit Sell: 50 YES @ 0.50 USDC");
    program.request()
        .accounts(nfl_blockchain::accounts::PlaceLimitSell {
            seller: alice.pubkey(),
            seller_token_ata: alice_yes,
            seller_receive_collateral_ata: alice_usdc,
            market_authority, 
            yes_vault: yes_vault_ata,
            no_vault: no_vault_ata,
            order_book: order_book_pda,
            market: market.pubkey(),
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::PlaceLimitSell { price: 500_000, quantity: 50, is_yes: true })
        .signer(&alice)
        .send().expect("Place limit sell failed");

    // --- Phase 2: Bob ---
    println!("\n--- Phase 2: Bob (Market Buyer) ---");
    let bob = Keypair::new();
    program.request().instruction(system_instruction::transfer(&payer.pubkey(), &bob.pubkey(), 1_000_000_000)).send().unwrap();
    let bob_usdc = spl_associated_token_account::get_associated_token_address(&bob.pubkey(), &base_mint.pubkey());
    let bob_yes = spl_associated_token_account::get_associated_token_address(&bob.pubkey(), &yes_mint.pubkey());

    program.request()
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &bob.pubkey(), &base_mint.pubkey(), &spl_token::id()))
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &bob.pubkey(), &yes_mint.pubkey(), &spl_token::id()))
        .instruction(spl_token::instruction::mint_to(&spl_token::id(), &base_mint.pubkey(), &bob_usdc, &payer.pubkey(), &[], 50_000_000).unwrap())
        .send().unwrap();

    println!("Bob Executes Market Buy: 20 YES");
    program.request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: bob.pubkey(),
            buyer_collateral_ata: bob_usdc,
            buyer_receive_token_ata: bob_yes,
            market: market.pubkey(),
            order_book: order_book_pda,
            yes_vault: yes_vault_ata,
            no_vault: no_vault_ata,
            market_authority,
            token_program: spl_token::id(),
        })
        .accounts(vec![AccountMeta::new(alice_usdc, false)]) // Alice's Payment Account
        .args(nfl_blockchain::instruction::MarketBuy { params: nfl_blockchain::MarketBuyParams { quantity: 20, want_yes: true } })
        .signer(&bob)
        .send().expect("Bob buy failed");

    print_balances(&program, base_mint.pubkey(), yes_mint.pubkey(), bob.pubkey(), "Bob Final");

    // =========================================================================
    // ACTOR 3: CHARLIE (Limit Buyer / Buy Exact)
    // =========================================================================
    println!("\n--- Phase 3: Charlie (Limit Buyer) ---");
    let charlie = Keypair::new();
    
    // 1. Fund Charlie
    program.request().instruction(system_instruction::transfer(&payer.pubkey(), &charlie.pubkey(), 1_000_000_000)).send().unwrap();

    // 2. Setup ATAs
    let charlie_usdc = spl_associated_token_account::get_associated_token_address(&charlie.pubkey(), &base_mint.pubkey());
    let charlie_yes = spl_associated_token_account::get_associated_token_address(&charlie.pubkey(), &yes_mint.pubkey());

    program.request()
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &charlie.pubkey(), &base_mint.pubkey(), &spl_token::id()))
        .instruction(spl_associated_token_account::instruction::create_associated_token_account(&payer.pubkey(), &charlie.pubkey(), &yes_mint.pubkey(), &spl_token::id()))
        .instruction(spl_token::instruction::mint_to(&spl_token::id(), &base_mint.pubkey(), &charlie_usdc, &payer.pubkey(), &[], 50_000_000).unwrap()) 
        .send().expect("Setup charlie ATAs");

    print_balances(&program, base_mint.pubkey(), yes_mint.pubkey(), charlie.pubkey(), "Charlie Init");

    println!("Charlie Executes Buy Exact (Limit): 10 YES @ Max 0.60 USDC");
    // Alice still has 30 YES left at 0.50 USDC.
    // Charlie is willing to pay up to 0.60.
    // Should match at 0.50.
    // Cost: 10 * 0.50 = 5.00 USDC.

    program.request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: charlie.pubkey(),
            buyer_collateral_ata: charlie_usdc,
            buyer_receive_token_ata: charlie_yes,
            market: market.pubkey(),
            order_book: order_book_pda,
            yes_vault: yes_vault_ata,
            no_vault: no_vault_ata,
            market_authority,
            token_program: spl_token::id(),
        })
        .accounts(vec![
            AccountMeta::new(alice_usdc, false) // Paying Alice
        ])
        .args(nfl_blockchain::instruction::BuyExact {
            params: nfl_blockchain::BuyExactParams {
                quantity: 10,
                want_yes: true,
                max_price: 600_000, // Willing to pay 0.60
            }
        })
        .signer(&charlie)
        .send().expect("Charlie buy exact failed");

    print_balances(&program, base_mint.pubkey(), yes_mint.pubkey(), charlie.pubkey(), "Charlie Final");
    
    // Checks:
    // Charlie spent 5_000_000. 50M - 5M = 45M.
    let charlie_usdc_acc: TokenAccount = program.account(charlie_usdc).unwrap();
    assert_eq!(charlie_usdc_acc.amount, 45_000_000);
    // Charlie has 10 YES.
    let charlie_yes_acc: TokenAccount = program.account(charlie_yes).unwrap();
    assert_eq!(charlie_yes_acc.amount, 10);

    // =========================================================================
    // FAILURE CASE: Low Bid
    // =========================================================================
    println!("\n--- Phase 4: Failure Test (Low Bid) ---");
    // Alice has 20 YES left at 0.50 USDC.
    // Charlie tries to buy 5 YES but max_price is 0.40 USDC.
    
    let should_fail = program.request()
        .accounts(nfl_blockchain::accounts::MarketBuyAccounts {
            buyer: charlie.pubkey(),
            buyer_collateral_ata: charlie_usdc,
            buyer_receive_token_ata: charlie_yes,
            market: market.pubkey(),
            order_book: order_book_pda,
            yes_vault: yes_vault_ata,
            no_vault: no_vault_ata,
            market_authority,
            token_program: spl_token::id(),
        })
        .accounts(vec![
            AccountMeta::new(alice_usdc, false)
        ])
        .args(nfl_blockchain::instruction::BuyExact {
            params: nfl_blockchain::BuyExactParams {
                quantity: 5,
                want_yes: true,
                max_price: 400_000, // Too low!
            }
        })
        .signer(&charlie)
        .send();

    assert!(should_fail.is_err(), "Buy should have failed due to price limit");
    println!("Low bid successfully rejected.");

    println!("\nTest Complete: All scenarios passed.");
}