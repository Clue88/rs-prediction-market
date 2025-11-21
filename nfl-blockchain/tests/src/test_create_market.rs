use std::str::FromStr;

// `system_instruction` and `system_program` are deprecated but I can't get the new one to work
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
fn test_create_market() {
    // Load payer from ANCHOR_WALLET
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let payer = read_keypair_file(&anchor_wallet).unwrap();

    // Initialize nfl-blockchain program
    let program_id = Pubkey::from_str("433xjq33NNMksxDcrSTqp42FcGc2MRYhHdoDPtiADHwc").unwrap();
    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program = client.program(program_id).unwrap();

    // Generate required accounts
    let market = Keypair::new();
    let base_mint = Keypair::new();
    let yes_mint = Keypair::new();
    let no_mint = Keypair::new();
    let vault = Keypair::new();

    // Derive PDA (market authority)
    let (market_authority, _bump) =
        Pubkey::find_program_address(&[b"market_auth", market.pubkey().as_ref()], &program_id);

    // Create and initialize base_mint
    let mint_rent = program
        .rpc()
        .get_minimum_balance_for_rent_exemption(Mint::LEN)
        .unwrap();

    // Create the base_mint account
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
        .expect("failed to create + init base mint");

    let system_program: Pubkey = Pubkey::new_from_array(system_program::id().to_bytes());

    let expiry_ts = 1_700_000_000i64;
    let tx = program
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
        .expect("");

    println!("Your transaction signature {}", tx);
}
