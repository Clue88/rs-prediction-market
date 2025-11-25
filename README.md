# NFL Prediction Market
This project implements a binary prediction market, similar to Kalshi or Polymarket.

## Contract Structure and Implementation
For each market, the program creates mints for YES and NO tokens and a vault for holding collateral.
Anyone can mint YES/NO pairs backed 1:1 by collateral. After the conclusion of the event, the market
can resolve the market, after which winners can redeem their tokens for the originally deposited
collateral.

Each market is represented by a `Market` account:
```rust
Market {
    authority,              // Admin/oracle
    base_mint,              // Collateral mint
    yes_mint,               // YES mint
    no_mint,                // NO mint
    vault,                  // Token account holding collateral
    expiry_ts,              // Earliest possible resolution time
    status,                 // Open / Halted / Resolved
    outcome,                // Pending / Yes / No / Invalid
    market_authority_bump   // Program Derived Address (PDA) bump
}
```

Trading occurs independently of the smart contracts themselves. For example, if Alice wants to trade
YES and Bob wants to trade NO, Alice could mint a YES/NO pair for 1 USDC and then sell the NO token
to Bob at 0.60 USDC. Then, if the outcome is YES, Alice has a profit of 0.60 USDC. If the outcome is
NO, Bob has a profit of 0.40 USDC.

## Installation
Install Solana:
```bash
curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash
```

Set up an Anchor wallet:
```bash
solana-keygen new
```

## Tests
```bash
cd nfl-blockchain
anchor test
```

A number of tests to test various parts of the market lifecycle are included in `tests/src`. The
tests cover the four instructions (`create_market`, `mint_pairs`, `resolve_market`, and `redeem`),
as well as some invariants (e.g., can't redeem twice, losers can't redeem).
