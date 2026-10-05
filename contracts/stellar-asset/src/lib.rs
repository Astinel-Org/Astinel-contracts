#![no_std]
use soroban_sdk::{
    Address, BytesN, Env, Symbol, contract, contractimpl, contracttype, symbol_short,
};

const BALANCE: Symbol = symbol_short!("BALANCE");
const ADMIN: Symbol = symbol_short!("ADMIN");
const DECIMALS: u32 = 7;
const TOTAL_SUPPLY: Symbol = symbol_short!("TOTAL_SUP");
const METADATA: Symbol = symbol_short!("METADATA");

#[contracttype]
#[derive(Clone, Debug)]
pub struct AssetMetadata {
    pub name: BytesN<32>,
    pub symbol: BytesN<4>,
    pub decimals: u32,
    pub total_supply: i128,
    pub issuer: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contract]
pub struct StellarAsset;

#[contractimpl]
impl StellarAsset {
    pub fn init(env: Env, admin: Address, name: BytesN<32>, symbol: BytesN<4>) {
        let metadata = AssetMetadata {
            name: name.clone(),
            symbol: symbol.clone(),
            decimals: DECIMALS,
            total_supply: 0,
            issuer: admin.clone(),
        };
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&METADATA, &metadata);
        env.storage().instance().set(&TOTAL_SUPPLY, &0_i128);
    }

    fn only_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        caller.require_auth();
        assert_eq!(caller.clone(), admin, "SAFE: caller is not admin");
    }

    pub fn metadata(env: &Env) -> AssetMetadata {
        env.storage().instance().get(&METADATA).unwrap()
    }

    pub fn name(env: &Env) -> BytesN<32> {
        Self::metadata(env).name
    }

    pub fn symbol(env: &Env) -> BytesN<4> {
        Self::metadata(env).symbol
    }

    pub fn decimals(_env: &Env) -> u32 {
        DECIMALS
    }

    pub fn balance(env: &Env, owner: Address) -> i128 {
        let key = (BALANCE, owner);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    fn set_balance(env: &Env, owner: &Address, amount: i128) {
        let key = (BALANCE, owner.clone());
        if amount == 0 {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &amount);
        }
    }

    pub fn total_supply(env: &Env) -> i128 {
        env.storage().instance().get(&TOTAL_SUPPLY).unwrap_or(0)
    }

    pub fn mint(env: Env, caller: Address, to: Address, amount: i128) {
        Self::only_admin(&env, &caller);
        assert!(amount > 0, "SAFE: amount must be positive");
        let bal = Self::balance(&env, to.clone());
        Self::set_balance(&env, &to, bal + amount);
        let supply = Self::total_supply(&env);
        env.storage()
            .instance()
            .set(&TOTAL_SUPPLY, &(supply + amount));
        let mut meta = Self::metadata(&env);
        meta.total_supply = supply + amount;
        env.storage().instance().set(&METADATA, &meta);
    }

    pub fn burn(env: Env, _caller: Address, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "SAFE: amount must be positive");
        let bal = Self::balance(&env, from.clone());
        assert!(bal >= amount, "SAFE: insufficient balance");
        Self::set_balance(&env, &from, bal - amount);
        let supply = Self::total_supply(&env);
        env.storage()
            .instance()
            .set(&TOTAL_SUPPLY, &(supply - amount));
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "SAFE: amount must be positive");
        let from_bal = Self::balance(&env, from.clone());
        assert!(from_bal >= amount, "SAFE: insufficient balance");
        let to_bal = Self::balance(&env, to.clone());
        Self::set_balance(&env, &from, from_bal - amount);
        Self::set_balance(&env, &to, to_bal + amount);
    }

    pub fn allowance(env: &Env, owner: Address, spender: Address) -> i128 {
        let key = (owner, spender);
        let allowance: Option<AllowanceValue> = env.storage().persistent().get(&key);
        match allowance {
            Some(a) if a.expiration_ledger < env.ledger().sequence() => 0,
            Some(a) => a.amount,
            None => 0,
        }
    }

    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        owner.require_auth();
        let key = (owner, spender);
        let allowance = AllowanceValue {
            amount,
            expiration_ledger,
        };
        env.storage().persistent().set(&key, &allowance);
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        assert!(amount > 0, "SAFE: amount must be positive");
        let allowed = Self::allowance(&env, from.clone(), spender.clone());
        assert!(allowed >= amount, "SAFE: insufficient allowance");
        let from_bal = Self::balance(&env, from.clone());
        assert!(from_bal >= amount, "SAFE: insufficient balance");
        let key = (from.clone(), spender);
        let existing: AllowanceValue = env.storage().persistent().get(&key).unwrap();
        let remaining = existing.amount - amount;
        let updated = AllowanceValue {
            amount: remaining,
            ..existing
        };
        env.storage().persistent().set(&key, &updated);
        Self::set_balance(&env, &from, from_bal - amount);
        Self::set_balance(&env, &to, Self::balance(&env, to.clone()) + amount);
    }

    pub fn admin(env: &Env) -> Address {
        env.storage().instance().get(&ADMIN).unwrap()
    }

    pub fn set_admin(env: Env, caller: Address, new_admin: Address) {
        Self::only_admin(&env, &caller);
        env.storage().instance().set(&ADMIN, &new_admin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn create_env() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(StellarAsset, ());
        (env, contract_id)
    }

    fn make_name(env: &Env) -> BytesN<32> {
        let mut arr = [0u8; 32];
        let name = b"Astinel Token";
        arr[..name.len()].copy_from_slice(name);
        BytesN::from_array(env, &arr)
    }

    fn make_symbol(env: &Env) -> BytesN<4> {
        BytesN::from_array(env, b"ASTL")
    }

    #[test]
    fn test_init() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            StellarAsset::init(
                env.clone(),
                admin.clone(),
                make_name(&env),
                make_symbol(&env),
            );
            assert_eq!(StellarAsset::decimals(&env), 7);
            assert_eq!(StellarAsset::total_supply(&env), 0);
        });
    }

    #[test]
    fn test_mint_and_transfer() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let alice = Address::generate(&env);
            let bob = Address::generate(&env);
            StellarAsset::init(
                env.clone(),
                admin.clone(),
                make_name(&env),
                make_symbol(&env),
            );
            StellarAsset::mint(env.clone(), admin.clone(), alice.clone(), 1000);
            assert_eq!(StellarAsset::balance(&env, alice.clone()), 1000);
            assert_eq!(StellarAsset::total_supply(&env), 1000);
            StellarAsset::transfer(env.clone(), alice.clone(), bob.clone(), 300);
            assert_eq!(StellarAsset::balance(&env, alice), 700);
            assert_eq!(StellarAsset::balance(&env, bob), 300);
        });
    }

    #[test]
    fn test_burn() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let user = Address::generate(&env);
            StellarAsset::init(
                env.clone(),
                admin.clone(),
                make_name(&env),
                make_symbol(&env),
            );
            StellarAsset::mint(env.clone(), admin.clone(), user.clone(), 1000);
            StellarAsset::burn(env.clone(), user.clone(), user.clone(), 400);
            assert_eq!(StellarAsset::balance(&env, user), 600);
            assert_eq!(StellarAsset::total_supply(&env), 600);
        });
    }
}
