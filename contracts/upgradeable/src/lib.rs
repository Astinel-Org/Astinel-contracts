#![no_std]
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, contract, contractimpl, symbol_short};

const ADMIN: Symbol = symbol_short!("ADMIN");
const IMPLEMENTATION: Symbol = symbol_short!("IMPL");
const VERSION: Symbol = symbol_short!("VERSION");

#[contract]
pub struct Upgradeable;

#[contractimpl]
impl Upgradeable {
    pub fn init(env: Env, admin: Address, wasm_hash: BytesN<32>, version: u32) {
        env.storage().instance().set(&ADMIN, &admin);
        #[cfg(not(test))]
        env.deployer()
            .update_current_contract_wasm(wasm_hash.clone());
        env.storage().instance().set(&IMPLEMENTATION, &wasm_hash);
        env.storage().instance().set(&VERSION, &version);
    }

    fn only_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        caller.require_auth();
        assert_eq!(caller.clone(), admin, "Upgradeable: caller is not admin");
    }

    pub fn admin(env: &Env) -> Address {
        env.storage().instance().get(&ADMIN).unwrap()
    }

    pub fn version(env: &Env) -> u32 {
        env.storage().instance().get(&VERSION).unwrap_or(1)
    }

    pub fn implementation(env: &Env) -> BytesN<32> {
        env.storage().instance().get(&IMPLEMENTATION).unwrap()
    }

    pub fn upgrade(env: Env, caller: Address, new_wasm_hash: BytesN<32>, new_version: u32) {
        Self::only_admin(&env, &caller);
        assert!(
            new_version > Self::version(&env),
            "Upgradeable: new version must be greater"
        );
        #[cfg(not(test))]
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        env.storage()
            .instance()
            .set(&IMPLEMENTATION, &new_wasm_hash);
        env.storage().instance().set(&VERSION, &new_version);
    }

    pub fn set_admin(env: Env, caller: Address, new_admin: Address) {
        Self::only_admin(&env, &caller);
        env.storage().instance().set(&ADMIN, &new_admin);
    }

    pub fn write_bytes(env: Env, caller: Address, key: Symbol, value: Bytes) {
        Self::only_admin(&env, &caller);
        env.storage().instance().set(&key, &value);
    }

    pub fn read_bytes(env: &Env, key: Symbol) -> Option<Bytes> {
        env.storage().instance().get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn create_env() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(Upgradeable, ());
        (env, contract_id)
    }

    #[test]
    fn test_init() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let wasm_hash = BytesN::from_array(&env, &[0u8; 32]);
            Upgradeable::init(env.clone(), admin.clone(), wasm_hash.clone(), 1);
            assert_eq!(Upgradeable::admin(&env), admin);
            assert_eq!(Upgradeable::version(&env), 1);
            assert_eq!(Upgradeable::implementation(&env), wasm_hash);
        });
    }

    #[test]
    fn test_upgrade() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let wasm_hash = BytesN::from_array(&env, &[0u8; 32]);
            let new_hash = BytesN::from_array(&env, &[1u8; 32]);
            Upgradeable::init(env.clone(), admin.clone(), wasm_hash, 1);
            Upgradeable::upgrade(env.clone(), admin.clone(), new_hash.clone(), 2);
            assert_eq!(Upgradeable::version(&env), 2);
            assert_eq!(Upgradeable::implementation(&env), new_hash);
        });
    }

    #[test]
    fn test_storage() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let wasm_hash = BytesN::from_array(&env, &[0u8; 32]);
            Upgradeable::init(env.clone(), admin.clone(), wasm_hash, 1);
            let key = symbol_short!("DATA1");
            let value = Bytes::from_array(&env, &[42]);
            Upgradeable::write_bytes(env.clone(), admin.clone(), key.clone(), value.clone());
            assert_eq!(Upgradeable::read_bytes(&env, key), Some(value));
        });
    }
}
