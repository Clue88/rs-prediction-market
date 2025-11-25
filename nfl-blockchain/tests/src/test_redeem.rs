use anchor_client::solana_sdk::signer::Signer;
use anchor_spl::token::{spl_token, TokenAccount};

use crate::test_utils::*;

#[test]
fn test_redeem_yes() {
    // Setup
    let (program, payer) = setup_client();

    let base_mint_kp = create_mint(&program, payer);
    let base_mint = base_mint_kp.pubkey();

    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) =
        create_market(&program, payer, base_mint);

    let user_collateral_ata = create_ata(&program, payer, payer.pubkey(), base_mint);
    mint_tokens(&program, payer, base_mint, user_collateral_ata, 100);

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
        10,
    );

    program
        .request()
        .accounts(nfl_blockchain::accounts::ResolveMarket {
            authority: payer.pubkey(),
            market: market_kp.pubkey(),
        })
        .args(nfl_blockchain::instruction::ResolveMarket {
            outcome: nfl_blockchain::Outcome::Yes,
        })
        .send()
        .unwrap();

    // Redeem YES tokens
    program
        .request()
        .accounts(nfl_blockchain::accounts::Redeem {
            user: payer.pubkey(),
            market: market_kp.pubkey(),
            base_mint,
            yes_mint: yes_mint_kp.pubkey(),
            no_mint: no_mint_kp.pubkey(),
            vault: vault_kp.pubkey(),
            user_collateral_ata,
            user_yes_ata,
            user_no_ata,
            market_authority,
            token_program: spl_token::id(),
        })
        .args(nfl_blockchain::instruction::Redeem {})
        .send()
        .unwrap();

    let vault_acc: TokenAccount = program.account(vault_kp.pubkey()).unwrap();
    let user_collateral: TokenAccount = program.account(user_collateral_ata).unwrap();
    let user_yes: TokenAccount = program.account(user_yes_ata).unwrap();
    let user_no: TokenAccount = program.account(user_no_ata).unwrap();

    assert_eq!(vault_acc.amount, 0); // vault fully emptied by redemption
    assert_eq!(user_collateral.amount, 100); // original 100 restored
    assert_eq!(user_yes.amount, 0); // winner tokens burned
    assert_eq!(user_no.amount, 10); // loser tokens unchanged

    println!("redeem YES test passed!");
}
