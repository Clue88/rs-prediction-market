use anchor_client::solana_sdk::signer::Signer;
use anchor_spl::token::TokenAccount;

use crate::test_utils::*;

#[test]
fn test_mint_pairs() {
    // Setup
    let (program, payer) = setup_client();

    let base_mint_kp = create_mint(&program, payer);
    let base_mint = base_mint_kp.pubkey();

    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) =
        create_market(&program, payer, base_mint);

    let user_collateral_ata = create_ata(&program, payer, payer.pubkey(), base_mint);
    mint_tokens(&program, payer, base_mint, user_collateral_ata, 100);

    // Mint `amount` YES/NO pairs
    let amount = 10;
    let (user_yes_ata, user_no_ata) = mint_pairs_for_user(
        &program,
        market_kp.pubkey(),
        base_mint,
        yes_mint_kp.pubkey(),
        no_mint_kp.pubkey(),
        vault_kp.pubkey(),
        market_authority,
        payer,
        user_collateral_ata,
        amount,
    );

    let vault_account: TokenAccount = program.account(vault_kp.pubkey()).unwrap();
    let user_yes: TokenAccount = program.account(user_yes_ata).unwrap();
    let user_no: TokenAccount = program.account(user_no_ata).unwrap();

    assert_eq!(vault_account.amount, amount);
    assert_eq!(user_yes.amount, amount);
    assert_eq!(user_no.amount, amount);

    println!("mint_pairs test passed!");
}
