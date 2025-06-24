## How to deploy
    git clone https://github.com/2xsolution/dedash-contract

    cd dex_raise

    npm install

    anchor build

    solana address -k target/deploy/dex_raise-keypair.json

And then you can get the address. This is program address.
You should copy it and then paste the exact place of the Anchor.toml and programs/ponzi/src/lib.rs.

Anchor.toml

    [programs.localnet]
    dex_raise = "BTorkvs6ZfddcEqcZqqd9EAT8thUBB2gE1ikmLH7m8i4"

    [programs.devnet]
    dex_raise = "6h6tptXmE1g9F6pez3yaZYxdvdkBQRFnw3VH1Fcs3uze"

    [programs.mainnet]
    dex_raise = "6h6tptXmE1g9F6pez3yaZYxdvdkBQRFnw3VH1Fcs3uze"

programs/dex_raise/src/lib.rs

    declare_id!("6h6tptXmE1g9F6pez3yaZYxdvdkBQRFnw3VH1Fcs3uze");

Finally please deploy the smart contract with the following:

    anchor build
    anchor deploy

## How to test

    anchor test