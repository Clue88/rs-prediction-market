use anchor_client::solana_sdk::{signature::Keypair, signer::Signer};
use anchor_spl::token::spl_token;

use crate::test_utils::*;

#[test]
fn test_loser_cannot_redeem() {
    // Setup
    let (program, payer) = setup_client();
    let user = payer.pubkey();

    let base_mint_kp = create_mint(&program, payer);
    let base_mint = base_mint_kp.pubkey();

    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, market_authority) =
        create_market(&program, payer, base_mint);

    let user_collateral = create_ata(&program, payer, user, base_mint);
    mint_tokens(&program, payer, base_mint, user_collateral, 100);

    let (_user_yes, user_no) = mint_pairs_for_user(
        &program,
        market_kp.pubkey(),
        base_mint,
        yes_mint_kp.pubkey(),
        no_mint_kp.pubkey(),
        vault_kp.pubkey(),
        market_authority,
        payer,
        user_collateral,
        10,
    );

    // Create loser user + ATAs
    let loser = Keypair::new();
    let loser_pubkey = loser.pubkey();

    let loser_collateral = create_ata(&program, payer, loser_pubkey, base_mint);
    let loser_yes = create_ata(&program, payer, loser_pubkey, yes_mint_kp.pubkey());
    let loser_no = create_ata(&program, payer, loser_pubkey, no_mint_kp.pubkey());

    // User transfers all NO tokens to loser
    let transfer_no_to_loser =
        spl_token::instruction::transfer(&spl_token::id(), &user_no, &loser_no, &user, &[], 10)
            .unwrap();
    program
        .request()
        .instruction(transfer_no_to_loser)
        .send()
        .unwrap();

    // Resolve market to YES
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

    // Loser tries to redeem (should fail)
    let result = program
        .request()
        .accounts(nfl_blockchain::accounts::Redeem {
            user: loser_pubkey,
            market: market_kp.pubkey(),
            base_mint,
            yes_mint: yes_mint_kp.pubkey(),
            no_mint: no_mint_kp.pubkey(),
            vault: vault_kp.pubkey(),
            user_collateral_ata: loser_collateral,
            user_yes_ata: loser_yes,
            user_no_ata: loser_no,
            market_authority,
            token_program: anchor_spl::token::spl_token::id(),
        })
        .args(nfl_blockchain::instruction::Redeem {})
        .signer(&loser)
        .send();
    assert!(result.is_err(), "Loser should not be able to redeem!");

    println!("Loser cannot redeem test passed!");
}
