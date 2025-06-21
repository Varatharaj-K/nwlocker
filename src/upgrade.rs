use crate::*;
use near_sdk::{env, near_bindgen, json_types::U128, Promise};

/// Upgrade-related methods
#[near_bindgen]
impl Contract {
    /// Called after code upgrade to restore state
    #[init(ignore_state)]
    #[private]
    pub fn migrate_state() -> Self {
        env::state_read().expect("ERR_NO_PREVIOUS_STATE")
    }

    /// Optional: View current contract version
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Helper to ensure only the contract owner can call
    pub fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "ERR_NOT_ALLOWED"
        );
    }

    /// Transfer NEAR tokens (only owner can call)
    #[payable]
    pub fn transfer_near(&mut self, recipient: AccountId, amount: U128) {
        self.assert_owner();
        Promise::new(recipient).transfer(amount.0);
    }
}

/// Entrypoint for contract upgrade using low-level env APIs
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn upgrade() {
    env::setup_panic_hook();

    // Restore the old state
    let contract: Contract = env::state_read().expect("ERR_NO_PREVIOUS_STATE");
    contract.assert_owner();

    // Read new code (passed as input)
    let new_code = env::input().expect("ERR_NO_INPUT");

    // Start a promise batch to this same account
    let promise_id = env::promise_batch_create(&env::current_account_id());

    // Deploy new code
    env::promise_batch_action_deploy_contract(promise_id, &new_code);

    // Call migrate_state to restore contract state
    env::promise_batch_action_function_call(
        promise_id,
        "migrate_state",
        b"{}",
        0,
        env::prepaid_gas() / 2,
    );

    // Return upgrade promise
    env::promise_return(promise_id);
}