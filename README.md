# Logos Oracle Network (LON)

## LEZ dev setup

* Base: Ubuntu 24.04 + rustup + docker

* to install: sudo apt install unzip python3.12-dev pkgconf libpcsclite-dev
* From tutorial.md in https://github.com/logos-co/spel/pull/138
  * RISC0 toolchain: https://dev.risczero.com/api/zkvm/install
  * Compile spel: `git clone https://github.com/logos-co/spel.git` && `cd spel` && `git checkout v0.6.0`  && `cargo build -p spel-framework -p spel-framework-core -p spel-framework-macros -p spel-client-gen -p spel`
  * Compile logos execution zone: `git clone https://github.com/logos-blockchain/logos-execution-zone.git && cd logos-execution-zone && git checkout v0.2.0`
    * Find the logos execution zone version in spel/spel-framework/Cargo.toml
    * Compile: `cargo build --release --features standalone -p sequencer_service` && `cargo build --release -p wallet`
  * Add spel & logos exec zone bin into $PATH: `vim ~/.bashrc` && set to `export PATH="$PATH:/home/ubuntu/.risc0/bin:/home/ubuntu/logos-execution-zone/target/release/:/home/ubuntu/spel/target/debug`
  * Test the setup: `wallet --version`, `spel --version`

## Launch the logos execution zone

RUST_LOG=info cargo run --features standalone -p sequencer_service -- lez/sequencer/service/configs/debug/sequencer_config.json

Note: 
* reset sequencer: `rm -rf logos-execution-zone/rocksdb`

## Launch the logos execution zone (LEZ v0.2.4)

From the `logos-execution-zone` repo (checked out at `v0.2.4`), in 2 terminals:
* Terminal 1 (Bedrock node, in docker): `just run-bedrock`
* Terminal 2 (LEZ sequencer, RPC on port 3040): `just run-sequencer`

Note:
* this replaces `docker compose up` (see "Logos blockchain setup" below, LEZ v0.2.0)
* wait for the sequencer to produce blocks before deploying contracts
* fresh restart: stop both, then `just clean` (removes sequencer / indexer rocksdb, bedrock signing key, debug wallet storage & bedrock docker volumes) and run both again

## Wallet

* Create a public account
  * `wallet account new public`
* List accounts
  * `wallet account list`

Note:
* delete wallet info: `rm -rf ~/.lee`

## Deploy contract & interact with

* make deploy
  * OR (manual way): `wallet deploy-program methods/guest/target/riscv32im-risc0-zkvm-elf/docker/my_counter.bin`
  * Sequencer logs: `Validated transaction with hash c8f138b1d34ba952978fcf86bcb53119c0c6f3ed10287bb36534de6d649f5aea, including it in block`
* init contract: 
  * `spel initialize --owner 5EYkqoY3fXNGqUABDMaCFurivdofeaXUofpKnJ6NrQE3`
* increment contract:
  * `spel increment --amount 5 --owner 5EYkqoY3fXNGqUABDMaCFurivdofeaXUofpKnJ6NrQE3`
* read counter:
  * `spel pda counter`
  * `spel inspect "G1gkRm62LdJ2XWpj5NBHeHgdNgjrzqQuPnW4CL8GqNjm" --type CounterState`

## Oracle register contract

### Build

* `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_register` then `make build`
* Faster dev build (still requires the CARGO_TARGET_DIR export):
  * `RISC0_USE_DOCKER=0 cargo build -j 8 --release`
* Generate idl
  * `spel generate-idl methods/guest/src/bin/oracle_register.rs > oracle_register-idl.json`

### Deploy

* Copy file
  * `cp -v /home/ubuntu/local_target/oracle_register/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin` 
  * dev build: `cp -v /home/ubuntu/local_target/oracle_register/riscv-guest/oracle_register-methods/oracle_register-guest/riscv32im-risc0-zkvm-elf/release/oracle_register.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin`
* Prerequisite: token program deployed + LON token created (see "LEZ token program" below)
* `make deploy`
* `spel initialize --owner C3EnT7M6kh3Dz3nX7FskfFDMk4mqNP6SLEtk34ZnLp8c --token-program-id 1824305940,902834625,3130365343,1050763279,2414906698,2830208766,2059686194,2825374910`
  * token program id: `spel program-id <token.bin> --format json` (use `program_id_decimal`)
  * owner: use the LON holding account (owned by the token program). With spel v0.7, a `#[account(signer)]` account that is not owned yet (e.g. a fresh wallet account) is claimed by the program
  * check: `spel pda register` then `spel inspect <REGISTER_PDA> --type RegisterState`* Register an oracle node:
  * Generate `pda_seed` + `to` account: `cd oracle_helper_1 && cargo run`
    * `to` = "compute vault pda", `pda_seed` = "vault pda seed bytes hex"
  * `spel register --token-def-account EFHm2oZxQoxAN8xPipuBxsTYt8Fe6Qa8aewE7dEPhwVA --oracle-key 0000000000000000000000000000000000000000000000000000000000000001 --from C3EnT7M6kh3Dz3nX7FskfFDMk4mqNP6SLEtk34ZnLp8c --to WaFJ7BesuVcMBB1oJpU3enbxoFsBahGNu3T6k2C3C7k --pda-seed d546e7902066da243a0efa4e4d716b7f78356fa6632adb563fb74ce0a0366d73`
    * `from`: the LON holding account, `token-def-account`: the LON token definition account
  * check the stake (`LON_STAKE_AMOUNT` LON moved from `from` to the vault): `spel --idl artifacts/token-idl.json -p <token.bin> inspect WaFJ7BesuVcMBB1oJpU3enbxoFsBahGNu3T6k2C3C7k --type TokenHolding` (from lez-programs)

### Generate client code

* `spel-client-gen --idl oracle_register-idl.json --out-dir ../oracle_register_client/src`

### oracle_register_client

* `cargo run -- /home/ubuntu/repos/logos_oracle_network/oracle_register/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin /home/ubuntu/lez-programs/programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin`

## LEZ token program

* doc: `https://github.com/logos-blockchain/lez-programs/tree/main/docs/token`

* `git clone https://github.com/logos-blockchain/lez-programs.git`
  * build: `cargo risczero build --manifest-path ./programs/token/methods/guest/Cargo.toml`
   * deploy: `wallet deploy-program programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin`
   * generate idl: `spel generate-idl programs/token/methods/guest/src/bin/token.rs > artifacts/token-idl.json`
   * See avail cmd: `spel --idl artifacts/token-idl.json --help`
   * create a token: `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- new-fungible-definition --name "LON" --total-supply 21000 --definition-target-account daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj --holding-target-account 9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp --mint-authority none`
     * Note: create accounts: `wallet account new public --label lon_token_def_account` && `wallet account new public --label lon_token_hold_account`
   * inspect:
     * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin inspect "9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp" --type TokenHolding`
     * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin inspect "daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj" --type TokenDefinition`
  * transfer:
    * Note: first is required a init account
    * `wallet account new public --label for_transfer_1`
    * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- initialize-account --account-to-initialize 58mmyXYGG4btrmFD1BwoY94BPAQ7MJE3x1hWugSCbChK --definition-account daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj`
    * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin inspect "58mmyXYGG4btrmFD1BwoY94BPAQ7MJE3x1hWugSCbChK" --type TokenHolding`
    * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- transfer --sender 9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp --recipient 58mmyXYGG4btrmFD1BwoY94BPAQ7MJE3x1hWugSCbChK --amount-to-transfer 10`

Tips:
* `spel program-id programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin`


## Resources

* lez-multisig: `https://github.com/logos-co/lez-multisig/blob/main/scripts/DEMO-RUNBOOK.md`

## oracle_prices contract

* Build
  * `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_prices`
  * `make build`
  * `cp -v ~/local_target/oracle_prices/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin`
  * `spel generate-idl methods/guest/src/bin/oracle_prices.rs > oracle_prices-idl.json`
  * Full rebuild:
    * `cargo clean && rm -rf methods/guest/target/ && docker builder prune -f && make build`
* Build (no docker)
  * `RISC0_USE_DOCKER=0 cargo build -j 8 --release`
  * `cp -v ~/local_target/oracle_prices/riscv-guest/oracle_prices-methods/oracle_prices-guest/riscv32im-risc0-zkvm-elf/release/oracle_prices.bin  methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin`
  * `spel generate-idl methods/guest/src/bin/oracle_prices.rs > oracle_prices-idl.json`
* Deploy
  * `make deploy`
* Init contract
  * `spel initialize`
* Init a feed (feed id: hex64 or string <= 32 chars)
  * `spel initialize-feed --feed-id manual_feed_1`
* Submit an attestation (price is pending until the dispute window is over)
  * `spel submit-attestation --feed-id manual_feed_1 --round 1 --price 1000 --decimals 8 --confidence 4242 --set-hash 0303030303030303030303030303030303030303030303030303030303030303 --proposer-oracle-id 0101010101010101010101010101010101010101010101010101010101010101 --proposer-channel-id 0202020202020202020202020202020202020202020202020202020202020202 --proposer-leaf-index 0 --proposer-membership-proof 0 --clock 4BdcjoXkq786TMWcBGGHqcxeLYMZmn17rL4eM9ZyRWNU`
    * `--clock`: LEZ clock account (`CLOCK_01_PROGRAM_ACCOUNT_ID`)
    * `--proposer-membership-proof`: not verified yet by the contract; spel cannot parse an empty `Vec<u8>` (`""`), so pass a dummy `0`
* Finalize the attestation (after `DISPUTE_WINDOW_BLOCKS` blocks, otherwise rejected)
  * `spel finalize-attestation --feed-id manual_feed_1 --round 1 --clock 4BdcjoXkq786TMWcBGGHqcxeLYMZmn17rL4eM9ZyRWNU`
* Get price (`pending` after submit, `last_final` after finalize)
  * `spel pda feed_price --feed-id manual_feed_1`
    * `spel inspect "D4hSX7eAK5NSqQjvcTXZYh4wESqfvFYYprTT8DRcxPJy" --type FeedState`
* Get feeds
  * `spel pda oracle_prices_account`
    * `spel inspect "BH33rRmEiN1J7EY6wzP3kPL8zCSpqdD3uKBG96EF2LQ" --type OraclePricesState`
* Note: `[u8; 32]` fields (feed_id, set_hash...) are displayed by `spel inspect` as account ids (`Public/...`)

### oracle_prices contract client

* `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_prices_client`
* build lib: `cargo build --release`
* run example: `cargo run -- ../oracle_prices/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin` 

## Logos blockchain setup

* `cd logos-execution-zone`
  * `docker compose up`
    * Modification done: 
      * in `bedrock/node-config.yaml` -> `filter: "!Env {}"`
      * in `docker-compose.override.yml` -> `RUST_LOG=info,overwatch=warn,overwatch::overwatch=warn`
  * reset the containers: `docker compose down -v`
* Setup
  * `sudo apt install protobuf-compiler`
* Setup lon repo
  * `git clone ...`
  * `git submodule update --init --recursive`
    * Fix submodule update: `cd oracle_node/logos-blockchain/ && git rm --cached .claude/worktrees/wf_d6259406-6a4-9`
* Run sequencer
  * Edit `resources/register_contract_config.toml` with some contract info
    * `oracle_register_program_id`: retrieved when oracle_register contract has been deployed AND initialized
    * `oracle_register_account`: the PDA where oracle_register has been deployed (find when init: `register → 5NTZHn2Q9AzT3GyVZcDkRCSmHz843Z43cxxLu3GUKjbk (PDA)`
    * `oracle_node_funding_account`: an account owned by oracle node that have some LON tokens (for staking)
    * `token_definition_account`: the account that hold the LON token definition 
  * `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_node`
  * `RUST_BACKTRACE=1 RUST_LOG="debug,hyper_util=info,rustls=info,h2=info" cargo run -p sequencer -- --data-folder /home/ubuntu/local_run/oracle_node/sequencer`
* Run indexer
  * `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_node`
  * `RUST_BACKTRACE=1 RUST_LOG="debug,hyper_util=info,rustls=info,h2=info" cargo run -p indexer`

## Swap demo contract

* `make build`
* `cp -v /home/ubuntu/local_target/swap_demo/riscv32im-risc0-zkvm-elf/docker/swap_demo.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/swap_demo.bin` 
* `spel generate-idl methods/guest/src/bin/swap_demo.rs > swap_demo-idl.json`

* `make deploy`
* `spel initialize --token-program-id 0,0...`
  * token program id can computed using: `lon_helpers` (FIXME / TODO: commit or find a better place)
    * `cd ../oracle_node` and `cargo run -p common --example print_program_id -- __HEX_STR__` (use hex string when program has been deployed) 

* init pool:
  * `cargo run -p pda_seed_tool -- __TOKEN_DEF_ACCOUNT__`
  * `spel initialize-pool --token-definition-account H85iyJ22t5gnB4bQnvgXmwKPQuuqe1KNxpyVDtLXtv8N --pool-pda-seed 99a4dafef6318dde772e6aeff09ab184bd517556260398e67d1bec3509f4c2b4 --pool-account 3TMVTWLozksatMSMHVeX8FnwNGMqcxncBUsG138KYnfu`

* swap:
  * `spel swap --amount 10 --to BYGSXyvQE28qCJstegnJWXwoxevwL48AsN7KZbgnG47U ...`


