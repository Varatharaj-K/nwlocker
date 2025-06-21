mod upgrade;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, near_bindgen, PublicKey, AccountId, require, Promise, PanicOnDefault, ONE_NEAR,
};

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault, Clone)]
pub struct Owner {
    pub owner: AccountId,
    pub time: u64,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pairs: (
        Option<(Owner, Option<Owner>)>,
        Option<(Owner, Option<Owner>)>,
        Option<(Owner, Option<Owner>)>,
        Option<(Owner, Option<Owner>)>,
        Option<(Owner, Option<Owner>)>,
    ),
    temp_keys: Vec<(PublicKey, u64)>, // (key, added_at)
    pub owner_id: AccountId
}

const ACCESS_KEY_LIFETIME_SECS: u64 = 300; // 30 minutes
const CONFIRMATION_WINDOW_SECS: u64 = 300;  // 5 minutes

fn check_owner_access(pair: &Option<(Owner, Option<Owner>)>, caller: &AccountId, now: u64, any_owner: bool) -> (bool, Option<(Owner, Option<Owner>)>) {
    match pair {
        Some((acc1, op_acc2)) => {
            let is_first_owner = acc1.owner == *caller;
            
            if any_owner && is_first_owner {
                return (true, None); // No state change needed for any_owner check
            }
            
            match op_acc2 {
                Some(acc2) => {
                    let is_second_owner = acc2.owner == *caller;
                    
                    if any_owner && is_second_owner {
                        return (true, None);
                    }
                    
                    // For unlock operations, need proper timing
                    if is_first_owner {
                        let mut new_acc1 = acc1.clone();
                        new_acc1.time = now;
                        let time_valid = now.saturating_sub(acc2.time) < CONFIRMATION_WINDOW_SECS;
                        let new_pair = (new_acc1, Some(acc2.clone()));
                        (time_valid, Some(new_pair))
                    } else if is_second_owner {
                        let mut new_acc2 = acc2.clone();
                        new_acc2.time = now;
                        let time_valid = now.saturating_sub(acc1.time) < CONFIRMATION_WINDOW_SECS;
                        let new_pair = (acc1.clone(), Some(new_acc2));
                        (time_valid, Some(new_pair))
                    } else {
                        (false, None)
                    }
                }
                None => {
                    // Single owner pair
                    if is_first_owner {
                        if any_owner {
                            (true, None)
                        } else {
                            let mut new_acc1 = acc1.clone();
                            new_acc1.time = now;
                            (true, Some((new_acc1, None)))
                        }
                    } else {
                        (false, None)
                    }
                }
            }
        }
        None => (false, None),
    }
}

// Fixed: Added missing function definition that was being called in unlock
fn get_owner(pair: &mut Option<(Owner, Option<Owner>)>, now: u64, any_owner: bool) -> bool {
    match pair {
        Some((acc1, op_acc2)) => {
            let is_first = acc1.owner == env::predecessor_account_id();
            if is_first {
                if any_owner {
                    return true;
                }
                acc1.time = now;
                env::log_str(&format!("Account {} updated time to {}", acc1.owner, now));
            }
            match op_acc2 {
                Some(acc2) => {
                    if acc2.owner == env::predecessor_account_id() {
                        if any_owner {
                            return true;
                        }
                        acc2.time = now;
                        env::log_str(&format!("Account {} updated time to {}", acc2.owner, now));
                        let time_diff = if now > acc1.time { now - acc1.time } else { 0 };
                        let access = is_first || time_diff < 300;
                        env::log_str(&format!("Time difference: {} seconds, access: {}", time_diff, access));
                        return access;
                    } else {
                        let time_diff = if now > acc2.time { now - acc2.time } else { 0 };
                        let access = is_first && time_diff < 300;
                        env::log_str(&format!("First account called, waiting for second. Time diff: {} seconds, access: {}", time_diff, access));
                        return access;
                    }
                }
                None => return is_first,
            }
        }
        None => false,
    }
}

#[near_bindgen]
impl Contract {
    #[init(ignore_state)]
    #[payable]
    pub fn init(pairs: Vec<(String, String)>) -> Self {
        require!(!pairs.is_empty(), "At least one pair must be provided");
        
        fn set_pair(owner_pair: Option<&(String, String)>) -> Option<(Owner, Option<Owner>)> {
            match owner_pair {
                Some(pair) => {
                    let first_account = pair.0.trim();
                    if first_account.is_empty() {
                        return None;
                    }
                    
                    // Validate account ID format
                    let acc1 = AccountId::new_unchecked(first_account.to_string());
                    let second_account = pair.1.trim();
                    
                    if second_account.is_empty() || second_account == first_account {
                        Some((Owner { owner: acc1, time: 0 }, None))
                    } else {
                        let acc2 = AccountId::new_unchecked(second_account.to_string());
                        Some((
                            Owner { owner: acc1, time: 0 },
                            Some(Owner { owner: acc2, time: 0 }),
                        ))
                    }
                }
                None => None,
            }
        }

        let caller = env::predecessor_account_id();
        
        // Validate that contract creator is not in any pair
        for (a1, a2) in &pairs {
            require!(
                a1.trim() != caller.as_str() && a2.trim() != caller.as_str(),
                "Contract creator cannot be set as an owner"
            );
        }

        let pair_data = (
            set_pair(pairs.get(0)),
            set_pair(pairs.get(1)),
            set_pair(pairs.get(2)),
            set_pair(pairs.get(3)),
            set_pair(pairs.get(4)),
        );

        // Count valid pairs
        let valid_pairs = [
            &pair_data.0, &pair_data.1, &pair_data.2, &pair_data.3, &pair_data.4
        ]
        .iter()
        .filter(|x| x.is_some())
        .count();

        require!(valid_pairs > 0, "At least one valid pair must be provided");

        // Calculate fee: 0.1 NEAR per additional pair beyond the first
        let fee = if valid_pairs > 1 {
            ((valid_pairs - 1) as u128 * ONE_NEAR) / 10
        } else {
            0
        };

        require!(
            env::attached_deposit() >= fee,
            &format!("Insufficient deposit. Required: {} yoctoNEAR", fee)
        );

        if fee > 0 {
            Promise::new(env::current_account_id()).transfer(fee);
        }

        Self {
            pairs: pair_data,
            temp_keys: vec![],
            owner_id: AccountId::new_unchecked("genesis.veax_dao.testnet".to_string())
        }
    }

    pub fn accounts_list(&self) -> Vec<(String, String)> {
        let pairs = [&self.pairs.0, &self.pairs.1, &self.pairs.2, &self.pairs.3, &self.pairs.4];
        
        pairs
            .iter()
            .map(|pair_opt| match pair_opt {
                Some((acc1, acc2_opt)) => {
                    let second_account = acc2_opt
                        .as_ref()
                        .map(|owner| owner.owner.to_string())
                        .unwrap_or_default();
                    (acc1.owner.to_string(), second_account)
                }
                None => (String::new(), String::new()),
            })
            .collect()
    }

    pub fn unlock(&mut self, public_key: PublicKey) -> bool {
        let now = env::block_timestamp() / 1_000_000_000;
        let caller = env::predecessor_account_id();
        
        // Check if key already exists
        if self.temp_keys.iter().any(|(key, _)| key == &public_key) {
            env::log_str("Public key already has access");
            return false;
        }
        
        // Fixed: Use proper method calls with Self::
        let mut result = false;
        
        result = get_owner(&mut self.pairs.0, now, false) || result;
        if !result {
            result = get_owner(&mut self.pairs.1, now, false) || result;
        }
        if !result {
            result = get_owner(&mut self.pairs.2, now, false) || result;
        }
        if !result {
            result = get_owner(&mut self.pairs.3, now, false) || result;
        }
        if !result {
            result = get_owner(&mut self.pairs.4, now, false) || result;
        }
        
        if result {
            Promise::new(env::current_account_id()).add_full_access_key(public_key.clone());
            self.temp_keys.push((public_key, now));
            env::log_str(&format!(
                "Access key granted to {} by {}",
                env::current_account_id(),
                caller
            ));
        } else {
            env::log_str(&format!(
                "Access denied for {}. Either not an owner or confirmation window expired.",
                caller
            ));
        }
        
        result
    }

    pub fn revoke_expired_keys(&mut self) {
        let now = env::block_timestamp() / 1_000_000_000;
        
        let (valid_keys, expired_keys): (Vec<_>, Vec<_>) = self
            .temp_keys
            .iter()
            .cloned()
            .partition(|(_, added_at)| now.saturating_sub(*added_at) < ACCESS_KEY_LIFETIME_SECS);

        // Remove expired keys
        for (key, added_at) in &expired_keys {
            Promise::new(env::current_account_id()).delete_key(key.clone());
            env::log_str(&format!(
                "Expired access key removed (age: {} seconds)",
                now.saturating_sub(*added_at)
            ));
        }

        self.temp_keys = valid_keys;
        
        env::log_str(&format!(
            "Key cleanup: {} expired keys removed, {} keys remaining",
            expired_keys.len(),
            self.temp_keys.len()
        ));
    }

    pub fn withdraw(&mut self, to: AccountId, amount: u128) {
        let now = env::block_timestamp() / 1_000_000_000;
        let caller = env::predecessor_account_id();
        
        let has_access = [&self.pairs.0, &self.pairs.1, &self.pairs.2, &self.pairs.3, &self.pairs.4]
            .iter()
            .any(|pair| {
                let (access, _) = check_owner_access(pair, &caller, now, true);
                access
            });

        require!(has_access, "Only an owner can withdraw funds");
        
        let contract_balance = env::account_balance();
        require!(
            amount <= contract_balance,
            &format!("Insufficient balance. Available: {} yoctoNEAR", contract_balance)
        );

        Promise::new(to.clone()).transfer(amount);
        
        env::log_str(&format!(
            "{} withdrew {} yoctoNEAR to {}",
            caller, amount, to
        ));
    }

    // View functions for debugging and monitoring
    pub fn get_temp_keys_count(&self) -> usize {
        self.temp_keys.len()
    }

    // pub fn get_temp_keys_info(&self) -> Vec<(String, u64)> {
    //     let now = env::block_timestamp() / 1_000_000_000;
    //     self.temp_keys
    //         .iter()
    //         .map(|(key, added_at)| {
    //             (
    //                 key.to_string(),
    //                 ACCESS_KEY_LIFETIME_SECS.saturating_sub(now.saturating_sub(*added_at))
    //             )
    //         })
    //         .collect()
    // }

    pub fn check_owner_status(&self, account: AccountId) -> Vec<(usize, bool, u64)> {
        let pairs = [&self.pairs.0, &self.pairs.1, &self.pairs.2, &self.pairs.3, &self.pairs.4];
        
        pairs
            .iter()
            .enumerate()
            .filter_map(|(idx, pair_opt)| {
                pair_opt.as_ref().and_then(|(acc1, acc2_opt)| {
                    if acc1.owner == account {
                        Some((idx, true, acc1.time))
                    } else if let Some(acc2) = acc2_opt {
                        if acc2.owner == account {
                            Some((idx, false, acc2.time))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    pub fn check_owner_status_old(&self, account: AccountId) -> Vec<(usize, bool, u64)> {
        let pairs = [&self.pairs.0, &self.pairs.1, &self.pairs.2, &self.pairs.3, &self.pairs.4];
        
        pairs
            .iter()
            .enumerate()
            .filter_map(|(idx, pair_opt)| {
                pair_opt.as_ref().and_then(|(acc1, acc2_opt)| {
                    if acc1.owner == account {
                        Some((idx, true, acc1.time))
                    } else if let Some(acc2) = acc2_opt {
                        if acc2.owner == account {
                            Some((idx, false, acc2.time))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}