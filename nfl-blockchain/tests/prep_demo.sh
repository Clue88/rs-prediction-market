
anchor build 
anchor deploy

solana airdrop 20 $(solana-keygen pubkey ~/.config/solana/marketauth.json) --url localhost     
solana airdrop 20 $(solana-keygen pubkey ~/.config/solana/user1.json) --url localhost
solana airdrop 20 $(solana-keygen pubkey ~/.config/solana/user2.json) --url localhost

spl-token wrap 10 ~/.config/solana/marketauth.json --url localhost
spl-token wrap 10 ~/.config/solana/user1.json --url localhost
spl-token wrap 10 ~/.config/solana/user2.json --url localhost


# "Log into" market authority account:
# export ANCHOR_WALLET=~/.config/solana/marketauth.json

# "Log into" user1 account:
# export ANCHOR_WALLET=~/.config/solana/user1.json

# "Log into" user2 account:
# export ANCHOR_WALLET=~/.config/solana/user2.json
