use spel_framework::prelude::*;
use sha2::{Sha256, Digest};

const LITERAL_POOL: &str = "swap_demo_pool";

fn pda_seed_bytes(literal: &str, def_account: &[u8; 32]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let tag = seed_from_str(literal);
    let mut hasher = Sha256::new();
    hasher.update(tag);
    hasher.update(def_account);
    hasher.finalize().into()
}

pub fn compute_pda(program_id: &ProgramId, literal: &str, def_account: &[u8; 32]) -> AccountId {
    let tag = seed_from_str(literal);
    compute_pda_multi(program_id, &[&tag as &dyn ToSeed, def_account])
}

use risc0_zkvm::compute_image_id;
use std::fs;
use anyhow::anyhow;

fn get_program_id_from_path(path: &str) -> [u32; 8] {
    let elf_bytes = fs::read(path).expect("Failed to read ELF file");
    let digest = compute_image_id(&elf_bytes).expect("Failed to compute Image ID");
    digest.into()
}

fn u32_8_to_hex(array: &[u32; 8]) -> String {

    let mut bytes = [0u8; 32];
    for (i, &val) in array.iter().enumerate() {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&val.to_le_bytes());
    }
    // Convert the 32 bytes into a 64-character hex string
    hex::encode(bytes)
}

fn main() -> anyhow::Result<()> {
    println!("Swap demo - account & pda seed computation:");

    let def_account_ = std::env::args().nth(1).unwrap();
    // let mut def_account = [0u8; 32];
    // hex::decode_to_slice(def_account_, &mut def_account)
    //    .map_err(|_| anyhow!("Invalid hex string or wrong length (must be exactly 64 hex characters)"))?;
    let def_account: [u8; 32] = bs58::decode(&def_account_)
        .into_vec()
        .expect("Cannot decode base 58")
        .try_into()
        .expect("Cannot convert base58 decoded bytes to [u8; 32]");

    let program_id_path = "methods/guest/target/riscv32im-risc0-zkvm-elf/docker/swap_demo.bin";
    println!("program path: {:?}", program_id_path);
    let program_id = get_program_id_from_path(program_id_path);
    println!("program id: {:?}", program_id);
    println!("program hex: {:?}", u32_8_to_hex(&program_id));

    let vault_pda_seed_bytes_ = pda_seed_bytes(LITERAL_POOL, &def_account);
    println!("pda seed bytes: {:?}", vault_pda_seed_bytes_);
    println!("pda seed bytes hex: {:?}", hex::encode(vault_pda_seed_bytes_));

    // println!("Computing vault pda (literal: {} -- key: {:?})", ORACLE_REGISER_LITERAL, create_key);
    println!("compute pda: {:?}", compute_pda(&program_id, LITERAL_POOL, &def_account));

    Ok(())
}