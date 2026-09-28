/// Generate IDL JSON for the swap_demo program.
///
/// Usage:
///   cargo run --bin generate_idl > swap_demo-idl.json

spel_framework::generate_idl!("../methods/guest/src/bin/swap_demo.rs");
