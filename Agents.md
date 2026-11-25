# LON (Local Oracle Network) Implementation

## 🎯 Project Context
This is a monorepo for a LON implementation. It consists of an off-chain Oracle Node (Rust workspace) and on-chain LEZ smart contracts. 

**Specification Document:** [Insert Link to Spec Here]

## 📂 Repository Structure
- `/oracle_node/` - Rust workspace containing the off-chain infrastructure (using [logos zone sdk](https://docs.logos.co/blockchain/zone-sdk/inscribe-data-on-chain-using-zone-sdk))
  - `sequencer/` - Rust crate responsible for ordering and batching oracle updates.
  - `indexer/` - Rust crate responsible for listening to on-chain events and indexing data.
  - `common/` - Rust crate to share some code between sequencer & indexer.
- `oracle_register` - LEZ Contract handling the registration (and staking) of oracle node sequencers.
- `oracle_register_client` - Rust crate (lib, auto-generated) to interact with oracle_register contract. Used by oracle_node.
- `oracle_prices` - LEZ Contract storing the pushed oracle price feeds (used by indexers).
- `oracle_prices_client` - Rust crate (lib, auto-generated) to interact with oracle_prices contract. Used by oracle_node.
- `swap_demo` - Demo LEZ contract utilizing the prices.

## 🛠️ Commands & Workflows
When modifying code, always verify your changes using the following project commands. 

### Rust Workspace (`oracle_node`)

Run all Cargo commands from inside the `/oracle_node` directory:
- **Build all crates:** `cargo build --workspace`
- **Run all tests:** `cargo test --workspace`
- **Linting:** `cargo clippy --workspace -- -D warnings`
- **Format:** `cargo fmt --all`
- **Run Sequencer:** TODO
- **Run Indexer:** TODO

### LEZ Smart Contracts

Contracts dev: with the [Logos spel framework](https://github.com/logos-co/spel) and ran over the [Logos execution zone](https://github.com/logos-blockchain/logos-execution-zone).

* oracle_register (in `oracle_register` folder)
  * Compile: `make build`
  * Generate idl: `spel generate-idl methods/guest/src/bin/oracle_register.rs > oracle_register-idl.json`
  * Deploy (require a running LEZ): `make deploy`
* oracle_prices (in `oracle_prices` folder)
  * Compile: `make build`
  * Generate idl: `spel generate-idl methods/guest/src/bin/oracle_prices.rs > oracle_prices-idl.json`
  * Deploy (require a running LEZ): `make deploy`
* swap_demo (in `swap_demo` folder)
  * Compile: `make build`
  * Generate idl: `spel generate-idl methods/guest/src/bin/swap_demo.rs > swap_demo-idl.json`
  * Deploy (require a running LEZ): `make deploy`

* Contracts e2e tests (require a running LEZ, do not run unless explicitly requested)
  * Tests live in the client crates (`tests/e2e.rs`), shared helpers in `e2e_utils`
  * oracle_register: `cd oracle_register_client && RUST_LOG=info cargo test --features e2e_lbc --test e2e -- --nocapture`
  * oracle_prices: `cd oracle_prices_client && RUST_LOG=info cargo test --features e2e_lbc --test e2e -- --nocapture`

### LEZ Smart Contracts client

Spel framework can generate contract client code automatically:
* oracle_register: `cd oracle_register && spel-client-gen --idl oracle_register-idl.json --out-dir ../oracle_register_client/src`. Update `../oracle_register_client/src/lib.rs` accordingly.
* oracle_prices: `cd oracle_prices && spel-client-gen --idl oracle_prices-idl.json --out-dir ../oracle_prices_client/src`. Update `../oracle_prices_client/src/lib.rs` accordingly.

## 🦀 Rust Workspace (`oracle_node`) Guidelines
- **Toolchain:** Use stable Rust (unless specified otherwise).
- **Style:** Strictly adhere to `cargo fmt` and `cargo clippy`. 
- **Architecture:** 
  - Keep the `sequencer` and `indexer` crates highly decoupled, put shared code in `common`. 
  - Favor asynchronous Rust (`tokio`) for network and I/O operations.
  - Use structured logging (e.g., `tracing` crate) for all node operations.

## 📜 LEZ Smart Contract Guidelines

Mental model is very similar to Solana (think account model, PDA, but difference is that call to other contracts are async).
