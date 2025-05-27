use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen, PublicKey, AccountId, require, Promise, ONE_NEAR, PanicOnDefault}; 


#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
struct Contract {
	pairs:	(Option<(Owner, Option<Owner>)>, Option<(Owner, Option<Owner>)>, Option<(Owner, Option<Owner>)>, Option<(Owner, Option<Owner>)>, Option<(Owner, Option<Owner>)>) 
}
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Owner {owner:AccountId, time:u64}

fn get_wallet(account: AccountId) -> AccountId {
	let s1 = account.to_string();
	let s2: String = s1.chars().skip(s1.len()-7).take(7).collect();
	match s2 == "testnet" {true => AccountId::new_unchecked("nwlocker.testnet".to_string()), false => AccountId::new_unchecked("nwlocker.near".to_string())}
}
fn get_owner(pair:&mut Option<(Owner, Option<Owner>)>, now:u64, any_owner:bool) -> bool {
	match pair {
		Some((acc1,op_acc2)) => {
			let is_first = acc1.owner == env::predecessor_account_id();
			if is_first {
				if any_owner {
					return true
				}
				acc1.time = now;
			}
			match op_acc2 {
				Some(acc2) => {
					if acc2.owner == env::predecessor_account_id() {
						if any_owner {
							return true
						}
						acc2.time = now;
						is_first || now - acc1.time < 300
					} else {
						is_first && now - acc2.time < 300
					}
				},
				None => is_first
			}
		},
		None => false
	}
}

#[near_bindgen]
impl Contract {
    #[private]	
    #[payable]
    #[init(ignore_state)]
    pub fn init(pairs:Vec<(String, String)>) -> Self {
		fn set_pair(owner_pair: Option<&(String, String)>) -> Option<(Owner, Option<Owner>)>{
			match owner_pair {
				Some(pair) => {
					if pair.0.to_string() == "" {
						return None
					}
					let second = pair.1.to_string();	
					if second == "" || second == pair.0.to_string() {
						Some((Owner {owner:AccountId::new_unchecked(pair.0.to_string()), time:0}, None))
					} else {
						Some((Owner {owner:AccountId::new_unchecked(pair.0.to_string()), time:0}, Some(Owner {owner:AccountId::new_unchecked(second), time:0})))
					}
				}, 
				None => None
			}
		}
		let acc_self = env::predecessor_account_id().to_string();
		for (acc1,acc2) in &pairs {
			if acc1.as_ref() == acc_self || acc2.as_ref() == acc_self {
				panic!("You cannot set the account name in the list of owners");
			}
		}
		let pairs = (set_pair(pairs.get(0)), set_pair(pairs.get(1)), set_pair(pairs.get(2)), set_pair(pairs.get(3)), set_pair(pairs.get(4)));
		let count = match pairs.0 {Some(_)=>1,None=>0} + match pairs.1 {Some(_)=>1,None=>0} + match pairs.2 {Some(_)=>1,None=>0} + match pairs.3 {Some(_)=>1,None=>0} + match pairs.4 {Some(_)=>1,None=>0};
		let mut fee = 0;
		if count>1 {
			fee = (count-1)*ONE_NEAR/10; 
		}	
		require!(fee <= env::attached_deposit(),"Not enough deposit");
		Promise::new(get_wallet(env::predecessor_account_id())).transfer(fee);
		Self {pairs:pairs}
    }
    pub fn accounts_list(&self) -> Vec<(String, String)> {
		let data = vec![self.pairs.0.as_ref(), self.pairs.1.as_ref(), self.pairs.2.as_ref(), self.pairs.3.as_ref(), self.pairs.4.as_ref()];
		data.into_iter().map(|x| 
			match x { 
				Some(pair) => {
					match pair.1.as_ref()  { 
						Some(acc2) => (pair.0.owner.to_string(), acc2.owner.to_string()),
						None => (pair.0.owner.to_string(), "".to_string())
					}
				}, 
				None => ("".to_string(), "".to_string())
			}
		).collect()
	}
    pub fn crowd_key(&mut self, public_key:PublicKey, allowance:Option<u8>){ 
		require!(env::predecessor_account_id() == env::current_account_id() || get_owner(&mut self.pairs.0,0,true) || get_owner(&mut self.pairs.1,0,true) || get_owner(&mut self.pairs.2,0,true) || get_owner(&mut self.pairs.3,0,true) || get_owner(&mut self.pairs.4,0,true), "for owners only");
		Promise::new(env::current_account_id()).add_access_key(public_key, match allowance { Some(val) => val as u128 * ONE_NEAR, None => ONE_NEAR}, AccountId::new_unchecked("app.nearcrowd.near".to_string()), "".to_string());
    }
    pub fn unlock(&mut self, public_key:PublicKey) -> bool { 
		let now = env::block_timestamp().to_string()[..10].parse::<u64>().unwrap();
		let mut res = get_owner(&mut self.pairs.0, now, false);
		res = get_owner(&mut self.pairs.1, now, false) || res;
		res = get_owner(&mut self.pairs.2, now, false) || res;
		res = get_owner(&mut self.pairs.3, now, false) || res;
		res = get_owner(&mut self.pairs.4, now, false) || res;
		if res {
			Promise::new(env::current_account_id()).add_full_access_key(public_key);
			env::log_str(&format!("@{} complete unlock @{} ", env::predecessor_account_id(), env::current_account_id()));					
		}
		res
    }
}
