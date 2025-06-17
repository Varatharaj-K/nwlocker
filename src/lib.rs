use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, near_bindgen, PublicKey, AccountId, require, Promise, PanicOnDefault, ONE_NEAR,
};

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
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
}

const ACCESS_KEY_LIFETIME_SECS: u64 = 1800;

fn get_owner(pair: &mut Option<(Owner, Option<Owner>)>, now: u64, any_owner: bool) -> bool {
    match pair {
        Some((acc1, op_acc2)) => {
            let is_first = acc1.owner == env::predecessor_account_id();
            if is_first {
                if any_owner {
                    return true;
                }
                acc1.time = now;
            }
            match op_acc2 {
                Some(acc2) => {
                    if acc2.owner == env::predecessor_account_id() {
                        if any_owner {
                            return true;
                        }
                        acc2.time = now;
                        is_first || now - acc1.time < 300
                    } else {
                        is_first && now - acc2.time < 300
                    }
                }
                None => is_first,
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
        fn set_pair(owner_pair: Option<&(String, String)>) -> Option<(Owner, Option<Owner>)> {
            match owner_pair {
                Some(pair) => {
                    if pair.0.trim().is_empty() {
                        return None;
                    }
                    let acc1 = AccountId::new_unchecked(pair.0.to_string());
                    let second = pair.1.trim().to_string();
                    if second.is_empty() || second == pair.0 {
                        Some((Owner { owner: acc1, time: 0 }, None))
                    } else {
                        Some((
                            Owner { owner: acc1, time: 0 },
                            Some(Owner { owner: AccountId::new_unchecked(second), time: 0 }),
                        ))
                    }
                }
                None => None,
            }
        }

        let caller = env::predecessor_account_id().to_string();
        for (a1, a2) in &pairs {
            require!(
                a1 != &caller && a2 != &caller,
                "You cannot set the contract creator as an owner"
            );
        }

        let pair_data = (
            set_pair(pairs.get(0)),
            set_pair(pairs.get(1)),
            set_pair(pairs.get(2)),
            set_pair(pairs.get(3)),
            set_pair(pairs.get(4)),
        );

        let count = [pair_data.0.as_ref(), pair_data.1.as_ref(), pair_data.2.as_ref(), pair_data.3.as_ref(), pair_data.4.as_ref()]
            .iter()
            .filter(|x| x.is_some())
            .count();

        let mut fee = 0;
        if count > 1 {
            fee = ((count - 1) as u128 * ONE_NEAR) / 10;
        }

        require!(env::attached_deposit() >= fee, "Not enough deposit");
        Promise::new(env::current_account_id()).transfer(fee);

        Self {
            pairs: pair_data,
            temp_keys: vec![],
        }
    }

    pub fn accounts_list(&self) -> Vec<(String, String)> {
        let data = vec![
            self.pairs.0.as_ref(),
            self.pairs.1.as_ref(),
            self.pairs.2.as_ref(),
            self.pairs.3.as_ref(),
            self.pairs.4.as_ref(),
        ];
        data.into_iter()
            .map(|x| match x {
                Some(pair) => {
                    let acc2 = pair.1.as_ref().map(|o| o.owner.to_string()).unwrap_or_default();
                    (pair.0.owner.to_string(), acc2)
                }
                None => ("".to_string(), "".to_string()),
            })
            .collect()
    }

    pub fn unlock(&mut self, public_key: PublicKey) -> bool {
        let now = env::block_timestamp() / 1_000_000_000;
        let mut res = false;

        res = get_owner(&mut self.pairs.0, now, false) || res;
        res = get_owner(&mut self.pairs.1, now, false) || res;
        res = get_owner(&mut self.pairs.2, now, false) || res;
        res = get_owner(&mut self.pairs.3, now, false) || res;
        res = get_owner(&mut self.pairs.4, now, false) || res;

        if res {
            Promise::new(env::current_account_id()).add_full_access_key(public_key.clone());
            self.temp_keys.push((public_key, now));
            env::log_str(&format!(
                "Access key granted to {} by {}",
                env::current_account_id(),
                env::predecessor_account_id()
            ));
        }

        res
    }

    pub fn revoke_expired_keys(&mut self) {
        let now = env::block_timestamp() / 1_000_000_000;
        let (valid, expired): (Vec<_>, Vec<_>) = self
            .temp_keys
            .iter()
            .cloned()
            .partition(|(_, added_at)| now - *added_at < ACCESS_KEY_LIFETIME_SECS);

        for (key, _) in expired {
            Promise::new(env::current_account_id()).delete_key(key);
        }

        self.temp_keys = valid;
    }

    pub fn withdraw(&mut self, to: AccountId, amount: u128) {
        let now = env::block_timestamp() / 1_000_000_000;
        require!(
            get_owner(&mut self.pairs.0, now, true)
                || get_owner(&mut self.pairs.1, now, true)
                || get_owner(&mut self.pairs.2, now, true)
                || get_owner(&mut self.pairs.3, now, true)
                || get_owner(&mut self.pairs.4, now, true),
            "Only an owner can withdraw"
        );

        Promise::new(to).transfer(amount);
    }
}