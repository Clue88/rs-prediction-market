use std::str::FromStr;

#[allow(deprecated)]
use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair},
        signer::Signer,
        system_instruction, system_program,
        sysvar::rent,
    },
    Client, Cluster, Program,
};
use anchor_spl::token::{spl_token, Mint};

/// Load payer from ANCHOR_WALLET and connect to localnet.
pub fn setup_client() -> (Program<&'static Keypair>, &'static Keypair) {
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let kp = read_keypair_file(anchor_wallet).unwrap();
    let boxed = Box::new(kp);
    let payer: &'static Keypair = Box::leak(boxed);

    let program_id = Pubkey::from_str("G4prxukWTw5gm6oRxpuKcrghNBAcKr64vr634LgRkdu8").unwrap();

    let client = Client::new_with_options(Cluster::Localnet, payer, CommitmentConfig::processed());
    let program = client.program(program_id).unwrap();

    (program, payer)
}

/// Create & initialize a mint with 6 decimals owned by `mint_authority`.
pub fn create_mint(program: &Program<&Keypair>, mint_authority: &Keypair) -> Keypair {
    let mint = Keypair::new();

    let mint_rent = program
        .rpc()
        .get_minimum_balance_for_rent_exemption(Mint::LEN)
        .unwrap();

    let create_ix = system_instruction::create_account(
        &mint_authority.pubkey(),
        &mint.pubkey(),
        mint_rent,
        Mint::LEN as u64,
        &spl_token::id(),
    );

    program
        .request()
        .instruction(create_ix)
        .instruction(
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &mint.pubkey(),
                &mint_authority.pubkey(),
                None,
                6,
            )
            .unwrap(),
        )
        .signer(&mint)
        .send()
        .unwrap();

    mint
}

/// Create an ATA for (owner, mint), paid by `payer`.
pub fn create_ata(
    program: &Program<&Keypair>,
    payer: &Keypair,
    owner: Pubkey,
    mint: Pubkey,
) -> Pubkey {
    let ata = spl_associated_token_account::get_associated_token_address(&owner, &mint);

    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        &owner,
        &mint,
        &spl_token::id(),
    );

    program.request().instruction(ix).send().unwrap();

    ata
}

/// Mint `amount` tokens of `mint` into `dest_ata`.
pub fn mint_tokens(
    program: &Program<&Keypair>,
    mint_authority: &Keypair,
    mint: Pubkey,
    dest_ata: Pubkey,
    amount: u64,
) {
    let ix = spl_token::instruction::mint_to(
        &spl_token::id(),
        &mint,
        &dest_ata,
        &mint_authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();

    program.request().instruction(ix).send().unwrap();
}

/// Create a market.
pub fn create_market(
    program: &Program<&Keypair>,
    payer: &Keypair,
    base_mint: Pubkey,
) -> (Keypair, Keypair, Keypair, Keypair, Pubkey) {
    let market = Keypair::new();
    let yes_mint = Keypair::new();
    let no_mint = Keypair::new();
    let vault = Keypair::new();

    let (market_authority, _bump) =
        Pubkey::find_program_address(&[b"market_auth", market.pubkey().as_ref()], &program.id());

    let system_program_pk = Pubkey::new_from_array(system_program::id().to_bytes());
    let expiry_ts = 1_700_000_000i64;

    program
        .request()
        .accounts(nfl_blockchain::accounts::CreateMarket {
            authority: payer.pubkey(),
            market: market.pubkey(),
            base_mint,
            yes_mint: yes_mint.pubkey(),
            no_mint: no_mint.pubkey(),
            vault: vault.pubkey(),
            market_authority,
            token_program: spl_token::id(),
            system_program: system_program_pk,
            rent: rent::id(),
        })
        .args(nfl_blockchain::instruction::CreateMarket { expiry_ts })
        .signer(&market)
        .signer(&yes_mint)
        .signer(&no_mint)
        .signer(&vault)
        .send()
        .unwrap();

    (market, yes_mint, no_mint, vault, market_authority)
}

/// Mint YES/NO pairs for `user`.
pub fn mint_pairs_for_user(
    program: &Program<&Keypair>,
    market: Pubkey,
    base_mint: Pubkey,
    yes_mint: Pubkey,
    no_mint: Pubkey,
    vault: Pubkey,
    market_authority: Pubkey,
    user: &Keypair,
    user_collateral_ata: Pubkey,
    amount: u64,
) -> (Pubkey, Pubkey) {
    let user_yes_ata = create_ata(program, user, user.pubkey(), yes_mint);
    let user_no_ata = create_ata(program, user, user.pubkey(), no_mint);

    program
        .request()
        .accounts(nfl_blockchain::accounts::MintPairs {
            user: user.pubkey(),
            user_collateral_ata,
            market,
            base_mint,
            yes_mint,
            no_mint,
            vault,
            user_yes_ata,
            user_no_ata,
            market_authority,
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::MintPairs { amount })
        .signer(user)
        .send()
        .unwrap();

    (user_yes_ata, user_no_ata)
}
