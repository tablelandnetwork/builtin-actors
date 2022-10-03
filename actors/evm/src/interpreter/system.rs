#![allow(dead_code)]

use fvm_ipld_hamt::Identity;

use {
    crate::interpreter::{StatusCode, U256},
    cid::Cid,
    fil_actors_runtime::{runtime::Runtime, ActorError},
    fvm_ipld_blockstore::Blockstore,
    fvm_ipld_hamt::Hamt,
};

/// The EVM stores its state as Key-Value pairs with both keys and values
/// being 256 bits long. We store thse in a HAMT, The keys are already hashed
/// by the Solidity compiler, so we can use the identity hasher.
pub type StateHamt<BS> = Hamt<BS, U256, U256, Identity>;

#[derive(Clone, Copy, Debug)]
pub enum StorageStatus {
    /// The value of a storage item has been left unchanged: 0 -> 0 and X -> X.
    Unchanged,
    /// The value of a storage item has been modified: X -> Y.
    Modified,
    /// A storage item has been modified after being modified before: X -> Y -> Z.
    ModifiedAgain,
    /// A new storage item has been added: 0 -> X.
    Added,
    /// A storage item has been deleted: X -> 0.
    Deleted,
}

/// Platform Abstraction Layer
/// that bridges the FVM world to EVM world
pub struct System<'r, BS: Blockstore, RT: Runtime<BS>> {
    pub rt: &'r mut RT,
    state: &'r mut StateHamt<BS>,
}

impl<'r, BS: Blockstore, RT: Runtime<BS>> System<'r, BS, RT> {
    pub fn new(rt: &'r mut RT, state: &'r mut StateHamt<BS>) -> anyhow::Result<Self> {
        Ok(Self { rt, state })
    }

    /// Reborrow the system with a shorter lifetime.
    #[allow(clippy::needless_lifetimes)]
    pub fn reborrow<'a>(&'a mut self) -> System<'a, BS, RT> {
        System { rt: &mut *self.rt, state: &mut *self.state }
    }

    pub fn flush_state(&mut self) -> Result<Cid, ActorError> {
        self.state.flush().map_err(|e| ActorError::illegal_state(e.to_string()))
    }

    /// Get value of a storage key.
    pub fn get_storage(&mut self, key: U256) -> Result<Option<U256>, StatusCode> {
        Ok(self.state.get(&key).map_err(|e| StatusCode::InternalError(e.to_string()))?.cloned())
    }

    /// Set value of a storage key.
    pub fn set_storage(
        &mut self,
        key: U256,
        value: Option<U256>,
    ) -> Result<StorageStatus, StatusCode> {
        let prev_value = self.get_storage(key)?;

        match (prev_value, value) {
            (None, None) => Ok(StorageStatus::Unchanged),
            (Some(_), None) => {
                self.state.delete(&key).map_err(|e| StatusCode::InternalError(e.to_string()))?;
                Ok(StorageStatus::Deleted)
            }
            (Some(p), Some(n)) if p == n => Ok(StorageStatus::Unchanged),
            (_, Some(v)) => {
                self.state.set(key, v).map_err(|e| StatusCode::InternalError(e.to_string()))?;
                if prev_value.is_none() {
                    Ok(StorageStatus::Added)
                } else {
                    Ok(StorageStatus::Modified)
                }
            }
        }
    }
}