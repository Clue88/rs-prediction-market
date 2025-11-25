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
fn test_resolve_market_yes() {
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

    // Resolve market to YES
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

    // Assert market updated
    let market_account: nfl_blockchain::Market = program.account(market.pubkey()).unwrap();

    assert_eq!(
        market_account.status,
        nfl_blockchain::MarketStatus::Resolved
    );
    assert_eq!(market_account.outcome, nfl_blockchain::Outcome::Yes);

    println!("resolve_market YES test passed!");
}
