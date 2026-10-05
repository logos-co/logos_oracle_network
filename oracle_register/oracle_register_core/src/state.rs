use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework_macros::account_type;
use crate::imt::{OracleMerkleTree, TREE_CAPACITY};

#[account_type]
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RegisterState {
    // /// The current count value.
    // pub count: u64,
    /// The owner
    pub owner: [u8; 32],
    pub token_program_id: [u32; 8],
    pub mtree: OracleMerkleTree,
    // Note: with tree depth of 10, this is 32 * 1024 -> 32Kb so ok
    pub registered: [[u8; 32]; TREE_CAPACITY],
}

impl Default for RegisterState {
    fn default() -> Self {
        Self {
            owner: [0; 32],
            token_program_id: [0; 8],
            mtree: Default::default(),
            registered: [[0u8; 32]; TREE_CAPACITY],
        }
    }
}

impl RegisterState {

    fn insert_oracle(&mut self, leaf: [u8; 32]) -> Result<(), &'static str> {
        let registered_index = self.mtree.len();
        self.mtree.insert_oracle(leaf)?;
        self.registered[registered_index] = leaf;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_merkle::{proof_serializers, MerkleTree};
    use crate::imt;
    use crate::rs_merkle_utils::Keccak256Algorithm;
    // use crate::Keccak256Algorithm;
    // use tiny_keccak::{Hasher as _, Keccak};

    #[test]
    fn rs_merkle_tree_proof() {

        // Check the RegisterState keep tracks of register oracles
        // and we can compute the correct merkle proof

        let mut reg_state = RegisterState::default();

        let leaf_1 = [1; 32];
        let leaf_2 = [42; 32];
        let leaf_3 = [33; 32];
        reg_state.insert_oracle(leaf_1).unwrap();
        reg_state.insert_oracle(leaf_2).unwrap();
        reg_state.insert_oracle(leaf_3).unwrap();

        assert_eq!(reg_state.mtree.next_index, 3);

        let mut reference_tree_leaves = vec![[0u8; 32]; imt::OracleMerkleTree::capacity()];
        reference_tree_leaves[0] = leaf_1;
        reference_tree_leaves[1] = leaf_2;
        reference_tree_leaves[2] = leaf_3;
        let mut reference_tree = MerkleTree::<Keccak256Algorithm>::from_leaves(&reference_tree_leaves);

        assert_eq!(reference_tree.root().unwrap(), reg_state.mtree.current_root);

        let leaf_indices = [0, 1];
        let reference_tree_mproof = reference_tree.proof(&leaf_indices);

        let proof_2 = {
            let mt = MerkleTree::<Keccak256Algorithm>::from_leaves(&reg_state.registered);
            mt.proof(&leaf_indices)
        };

        assert_eq!(
            reference_tree_mproof.serialize::<proof_serializers::DirectHashesOrder>(),
            proof_2.serialize::<proof_serializers::DirectHashesOrder>()
        );

        // TODO
        // let mproof_root = reference_tree_mproof.root(&[0, 1], );
    }

}