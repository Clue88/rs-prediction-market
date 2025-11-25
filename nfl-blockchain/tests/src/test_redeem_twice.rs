use std::str::FromStr;

#[allow(deprecated)]
use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
        signature::Keypair, signer::Signer, system_instruction, system_program, sysvar::rent,
    },
    Client, Cluster,
};
use anchor_spl::token::{spl_token, Mint};

#[test]
fn test_redeem_twice_should_fail() {
    // Setup client and payer
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let payer = read_keypair_file(&anchor_wallet).unwrap();

    let program_id = Pubkey::from_str("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc").unwrap();
    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::processed());
    let program = client.program(program_id).unwrap();

    // Create accounts
    let market = Keypair::new();
    let base_mint = Keypair::new();
    let yes_mint = Keypair::new();
    let no_mint = Keypair::new();
    let vault = Keypair::new();

    let (market_authority, _bump) =
        Pubkey::find_program_address(&[b"market_auth", market.pubkey().as_ref()], &program_id);

    // Create base mint
    let mint_rent = program
        .rpc()
        .get_minimum_balance_for_rent_exemption(Mint::LEN)
        .unwrap();
    let create_mint_ix = system_instruction::create_account(
        &payer.pubkey(),
        &base_mint.pubkey(),
        mint_rent,
        Mint::LEN as u64,
        &spl_token::id(),
    );
    program
        .request()
        .instruction(create_mint_ix)
        .instruction(
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &base_mint.pubkey(),
                &payer.pubkey(),
                None,
                6,
            )
            .unwrap(),
        )
        .signer(&base_mint)
        .send()
        .unwrap();

    // Create market
    let system_program = Pubkey::new_from_array(system_program::id().to_bytes());
    let expiry_ts = 1_700_000_000i64;

    program
        .request()
        .accounts(nfl_blockchain::accounts::CreateMarket {
            authority: payer.pubkey(),
            market: market.pubkey(),
            base_mint: base_mint.pubkey(),
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            market_authority,
            token_program: spl_token::id(),
            system_program,
            rent: rent::id(),
        })
        .args(nfl_blockchain::instruction::CreateMarket { expiry_ts })
        .signer(&market)
        .signer(&yes_mint)
        .signer(&no_mint)
        .signer(&vault)
        .send()
        .unwrap();

    // Create ATAs
    let user = payer.pubkey();
    let user_collateral =
        spl_associated_token_account::get_associated_token_address(&user, &base_mint.pubkey());
    let user_yes =
        spl_associated_token_account::get_associated_token_address(&user, &yes_mint.pubkey());
    let user_no =
        spl_associated_token_account::get_associated_token_address(&user, &no_mint.pubkey());

    for ix in [
        spl_associated_token_account::instruction::create_associated_token_account(
            &user,
            &user,
            &base_mint.pubkey(),
            &spl_token::id(),
        ),
        spl_associated_token_account::instruction::create_associated_token_account(
            &user,
            &user,
            &yes_mint.pubkey(),
            &spl_token::id(),
        ),
        spl_associated_token_account::instruction::create_associated_token_account(
            &user,
            &user,
            &no_mint.pubkey(),
            &spl_token::id(),
        ),
    ] {
        program.request().instruction(ix).send().unwrap();
    }

    // Mint collateral
    let mint_to_user = spl_token::instruction::mint_to(
        &spl_token::id(),
        &base_mint.pubkey(),
        &user_collateral,
        &payer.pubkey(),
        &[],
        100,
    )
    .unwrap();
    program.request().instruction(mint_to_user).send().unwrap();

    // Mint pairs
    program
        .request()
        .accounts(nfl_blockchain::accounts::MintPairs {
            user,
            user_collateral_ata: user_collateral,
            market: market.pubkey(),
            base_mint: base_mint.pubkey(),
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            user_yes_ata: user_yes,
            user_no_ata: user_no,
            market_authority,
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MintPairs { amount: 10 })
        .send()
        .unwrap();

    // Resolve to YES
    program
        .request()
        .accounts(nfl_blockchain::accounts::ResolveMarket {
            authority: payer.pubkey(),
            market: market.pubkey(),
        })
        .args(nfl_blockchain::instruction::ResolveMarket {
            outcome: nfl_blockchain::Outcome::Yes,
        })
        .send()
        .unwrap();

    // First redemption: should succeed
    program
        .request()
        .accounts(nfl_blockchain::accounts::Redeem {
            user,
            market: market.pubkey(),
            base_mint: base_mint.pubkey(),
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            user_collateral_ata: user_collateral,
            user_yes_ata: user_yes,
            user_no_ata: user_no,
            market_authority,
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::Redeem {})
        .send()
        .unwrap();

    // Second redemption: should fail
    let result = program
        .request()
        .accounts(nfl_blockchain::accounts::Redeem {
            user,
            market: market.pubkey(),
            base_mint: base_mint.pubkey(),
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            user_collateral_ata: user_collateral,
            user_yes_ata: user_yes,
            user_no_ata: user_no,
            market_authority,
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::Redeem {})
        .send();

    assert!(result.is_err(), "Second redeem should fail!");

    println!("Redeem twice test passed!");
}
