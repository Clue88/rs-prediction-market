use anchor_client::solana_sdk::signer::Signer;

use crate::test_utils::*;

#[test]
fn test_create_market() {
    let (program, payer) = setup_client();

    let base_mint_kp = create_mint(&program, payer);
    let base_mint = base_mint_kp.pubkey();

    let (market_kp, yes_mint_kp, no_mint_kp, vault_kp, _market_authority) =
        create_market(&program, payer, base_mint);

    let market_account: nfl_blockchain::Market = program.account(market_kp.pubkey()).unwrap();

    assert_eq!(market_account.base_mint, base_mint);
    assert_eq!(market_account.yes_mint, yes_mint_kp.pubkey());
    assert_eq!(market_account.no_mint, no_mint_kp.pubkey());
    assert_eq!(market_account.vault, vault_kp.pubkey());
    assert_eq!(market_account.authority, payer.pubkey());
    assert_eq!(market_account.status, nfl_blockchain::MarketStatus::Open);
    assert_eq!(market_account.outcome, nfl_blockchain::Outcome::Pending);

    println!("create_market test passed!");
}
