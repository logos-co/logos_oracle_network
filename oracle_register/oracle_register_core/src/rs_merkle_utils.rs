use rs_merkle::{Hasher, MerkleTree};
use tiny_keccak::{Hasher as KeccakHasher, Keccak};

#[derive(Clone, Debug)]
pub struct Keccak256Algorithm;

// Implement the rs_merkle Hasher trait using Keccak256
impl Hasher for Keccak256Algorithm {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Keccak::v256();
        hasher.update(data);
        let mut output = [0u8; 32];
        hasher.finalize(&mut output);
        output
    }
}