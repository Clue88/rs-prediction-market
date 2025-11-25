use anchor_client::solana_sdk::signer::Signer;

use crate::test_utils::*;

#[test]
fn test_resolve_market_yes() {
    // Setup
    let (program, payer) = setup_client();

    let base_mint_kp = create_mint(&program, payer);
    let base_mint = base_mint_kp.pubkey();

    let (market_kp, _yes_mint_kp, _no_mint_kp, _vault_kp, _market_authority) =
        create_market(&program, payer, base_mint);

    // Resolve market to YES
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

    let market_account: nfl_blockchain::Market = program.account(market_kp.pubkey()).unwrap();

    assert!(market_account.status == nfl_blockchain::MarketStatus::Resolved);
    assert!(market_account.outcome == nfl_blockchain::Outcome::Yes);

    println!("resolve_market YES test passed!");
}
