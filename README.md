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

## Trading Mechanism and Settlement
Trading is facilitated by an on-chain order book. While users can mint pairs 1:1, the order book 
allows them to isolate their risk to a single outcome by selling the opposing token.

Example Scenario: Alice wants to bet on YES. She initiates a trade by minting a YES/NO pair for 1 USDC. 
She then places a limit order to sell the NO token for 0.60 USDC. This instruction holds her NO token 
in escrow until a buyer is found.
* The Trade: Bob believes the event will not happen (NO). He accepts Alice's price. 
The system instantly transfers Bob's 0.60 USDC to Alice and releases the NO token to Bob.
* The Positions:
    * Alice now holds a YES token. She paid 1.00 originally but received 0.60 back. 
    Her effective cost for the YES position is 0.40 USDC.
    * Bob now holds a NO token. He paid 0.60 USDC for it.
* Resolution: Once the event concludes, the market resolves to a final outcome, 
unlocking the collateral vault for the winning side only:
    * If YES wins: The YES token becomes redeemable for 1.00 USDC. 
    Alice profits 0.60 (1.00 payout - 0.40 cost). Bob's NO token becomes worthless.
    * If NO wins: The NO token becomes redeemable for 1.00 USDC. 
    Bob profits 0.40 (1.00 payout - 0.60 cost). Alice's YES token becomes worthless.

Each order book is represented by an `OrderBook` account and each order within the order book is represented by an `Order` struct:
```rust
#[account]
pub struct OrderBook {
    pub market: Pubkey,        // Market address
    pub next_order_id: u64,    // Unique ID counter
    pub capacity: u64,         // Max active orders
    pub orders: Vec<Order>,    // List of open orders
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Order {
    pub id: u64,                               // Order ID
    pub owner: Pubkey,                         // Seller wallet
    pub seller_receive_collateral_ata: Pubkey, // Seller payment account
    pub price: u64,                            // Ask price per share
    pub quantity: u64,                         // Amount to sell
    pub is_yes: bool,                          // YES token if true, NO o/w
}
```

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
tests cover the core market instructions (`create_market`, `mint_pairs`, `resolve_market`, and `redeem`), 
the order book exchange mechanism (`initialize_order_book`, `place_limit_sell`, `market_buy`, and `buy_exact`), 
and some invariants (e.g., can't redeem twice, losers can't redeem). 

Tests may fail if executed in parallel. If tests failing at first, try the following command:

```bash
RUST_TEST_THREADS=1 anchor test
```

