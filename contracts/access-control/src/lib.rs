#![no_std]
use soroban_sdk::{Address, Env, Symbol, Vec, contract, contractimpl, symbol_short};

const ADMIN: Symbol = symbol_short!("ADMIN");
const OPERATORS: Symbol = symbol_short!("OPRTRS");
const PAUSED: Symbol = symbol_short!("PAUSED");

fn only_admin(env: &Env, caller: &Address) {
    let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
    caller.require_auth();
    assert_eq!(caller.clone(), admin, "AccessControl: caller is not admin");
}

pub fn only_operator_or_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
    if caller.clone() == admin {
        return;
    }
    let operators: Vec<Address> = env.storage().instance().get(&OPERATORS).unwrap();
    assert!(
        operators.contains(caller),
        "AccessControl: caller is not operator or admin"
    );
}

#[contract]
pub struct AccessControl;

#[contractimpl]
impl AccessControl {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&PAUSED, &false);
        let empty: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&OPERATORS, &empty);
    }

    pub fn admin(env: &Env) -> Address {
        env.storage().instance().get(&ADMIN).unwrap()
    }

    pub fn set_admin(env: Env, caller: Address, new_admin: Address) {
        only_admin(&env, &caller);
        env.storage().instance().set(&ADMIN, &new_admin);
    }

    pub fn operators(env: &Env) -> Vec<Address> {
        env.storage().instance().get(&OPERATORS).unwrap()
    }

    pub fn add_operator(env: Env, caller: Address, operator: Address) {
        only_admin(&env, &caller);
        let mut ops: Vec<Address> = env.storage().instance().get(&OPERATORS).unwrap();
        ops.push_back(operator);
        env.storage().instance().set(&OPERATORS, &ops);
    }

    pub fn remove_operator(env: Env, caller: Address, operator: Address) {
        only_admin(&env, &caller);
        let ops: Vec<Address> = env.storage().instance().get(&OPERATORS).unwrap();
        let mut filtered: Vec<Address> = Vec::new(&env);
        for a in ops.iter() {
            if a != operator {
                filtered.push_back(a);
            }
        }
        env.storage().instance().set(&OPERATORS, &filtered);
    }

    pub fn paused(env: &Env) -> bool {
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    pub fn set_paused(env: Env, caller: Address, paused: bool) {
        only_admin(&env, &caller);
        env.storage().instance().set(&PAUSED, &paused);
    }

    pub fn require_not_paused(env: &Env) {
        assert!(!Self::paused(env), "AccessControl: contract is paused");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn create_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let contract_id = env.register(AccessControl, ());
        (env, contract_id)
    }

    #[test]
    fn test_init_and_admin() {
        let (env, contract_id) = create_env();
        env.as_contract(&contract_id, || {
            let admin = soroban_sdk::Address::generate(&env);
            AccessControl::init(env.clone(), admin.clone());
            assert_eq!(AccessControl::admin(&env), admin);
        });
    }

    #[test]
    fn test_add_operator() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = soroban_sdk::Address::generate(&env);
            let operator = soroban_sdk::Address::generate(&env);
            AccessControl::init(env.clone(), admin.clone());
            AccessControl::add_operator(env.clone(), admin.clone(), operator.clone());
            let ops = AccessControl::operators(&env);
            assert!(ops.contains(&operator));
        });
    }

    #[test]
    fn test_pause() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = soroban_sdk::Address::generate(&env);
            AccessControl::init(env.clone(), admin.clone());
            assert!(!AccessControl::paused(&env));
            AccessControl::set_paused(env.clone(), admin.clone(), true);
            assert!(AccessControl::paused(&env));
        });
    }
}
