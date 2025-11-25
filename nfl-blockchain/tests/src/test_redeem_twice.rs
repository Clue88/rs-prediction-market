use anchor_client::solana_sdk::signer::Signer;
use anchor_spl::token::spl_token;

use crate::test_utils::*;

#[test]
fn test_redeem_twice_should_fail() {
    // Setup
    let (program, payer) = setup_client();
    let user = payer.pubkey();

    let base_mint_kp = create_mint(&program, payer);
    let base_mint = base_mint_kp.pubkey();

    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) =
        create_market(&program, payer, base_mint);

    let user_collateral_ata = create_ata(&program, payer, user, base_mint);
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
            authority: user,
            market: market_kp.pubkey(),
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

    // Second redemption: should fail
    let result = program
        .request()
        .accounts(nfl_blockchain::accounts::Redeem {
            user,
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
        .send();
    assert!(result.is_err(), "Second redemption should fail");

    println!("Redeem twice test passed!");
}
