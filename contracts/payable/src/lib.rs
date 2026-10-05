#![no_std]
use soroban_sdk::{Address, Env, Symbol, contract, contractimpl, contracttype, symbol_short};

const TOTAL_SUPPLY: Symbol = symbol_short!("TOTLSUP");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contract]
pub struct Payable;

#[contractimpl]
impl Payable {
    pub fn init(env: Env, admin: Address) {
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "ADMIN"), &admin);
        env.storage().instance().set(&TOTAL_SUPPLY, &0_i128);
    }

    pub fn admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&Symbol::new(env, "ADMIN"))
            .unwrap()
    }

    fn only_admin(env: &Env, caller: &Address) {
        let admin: Address = Self::admin(env);
        caller.require_auth();
        assert_eq!(caller.clone(), admin, "Payable: caller is not admin");
    }

    pub fn balance(env: &Env, owner: Address) -> i128 {
        env.storage()
            .persistent()
            .get::<Address, i128>(&owner)
            .unwrap_or(0)
    }

    fn set_balance(env: &Env, owner: &Address, amount: i128) {
        if amount == 0 {
            env.storage().persistent().remove(owner);
        } else {
            env.storage().persistent().set(owner, &amount);
        }
    }

    pub fn total_supply(env: &Env) -> i128 {
        env.storage().instance().get(&TOTAL_SUPPLY).unwrap_or(0)
    }

    pub fn mint(env: Env, caller: Address, to: Address, amount: i128) {
        Self::only_admin(&env, &caller);
        assert!(amount > 0, "Payable: amount must be positive");
        let bal = Self::balance(&env, to.clone());
        Self::set_balance(&env, &to, bal + amount);
        let supply = Self::total_supply(&env);
        env.storage()
            .instance()
            .set(&TOTAL_SUPPLY, &(supply + amount));
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "Payable: amount must be positive");
        let from_bal = Self::balance(&env, from.clone());
        assert!(from_bal >= amount, "Payable: insufficient balance");
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
        assert!(amount >= 0, "Payable: amount must be non-negative");
        let key = (owner, spender);
        let allowance = AllowanceValue {
            amount,
            expiration_ledger,
        };
        env.storage().persistent().set(&key, &allowance);
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        assert!(amount > 0, "Payable: amount must be positive");
        let mut allowed = Self::allowance(&env, from.clone(), spender.clone());
        assert!(allowed >= amount, "Payable: insufficient allowance");
        let from_bal = Self::balance(&env, from.clone());
        assert!(from_bal >= amount, "Payable: insufficient balance");
        allowed -= amount;
        let key = (from.clone(), spender);
        let existing: AllowanceValue = env.storage().persistent().get(&key).unwrap();
        let updated = AllowanceValue {
            amount: allowed,
            ..existing
        };
        env.storage().persistent().set(&key, &updated);
        let to_bal = Self::balance(&env, to.clone());
        Self::set_balance(&env, &from, from_bal - amount);
        Self::set_balance(&env, &to, to_bal + amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn create_env() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(Payable, ());
        (env, contract_id)
    }

    #[test]
    fn test_mint_and_balance() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let user = Address::generate(&env);
            Payable::init(env.clone(), admin.clone());
            Payable::mint(env.clone(), admin.clone(), user.clone(), 1000);
            assert_eq!(Payable::balance(&env, user), 1000);
            assert_eq!(Payable::total_supply(&env), 1000);
        });
    }

    #[test]
    fn test_transfer() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let alice = Address::generate(&env);
            let bob = Address::generate(&env);
            Payable::init(env.clone(), admin.clone());
            Payable::mint(env.clone(), admin.clone(), alice.clone(), 1000);
            Payable::transfer(env.clone(), alice.clone(), bob.clone(), 500);
            assert_eq!(Payable::balance(&env, alice), 500);
            assert_eq!(Payable::balance(&env, bob), 500);
        });
    }

    #[test]
    fn test_allowance() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let owner = Address::generate(&env);
            let spender = Address::generate(&env);
            Payable::init(env.clone(), admin.clone());
            Payable::mint(env.clone(), admin.clone(), owner.clone(), 1000);
            let expiration = env.ledger().sequence() + 1000;
            Payable::approve(env.clone(), owner.clone(), spender.clone(), 500, expiration);
            assert_eq!(
                Payable::allowance(&env, owner.clone(), spender.clone()),
                500
            );
            Payable::transfer_from(
                env.clone(),
                spender.clone(),
                owner.clone(),
                admin.clone(),
                200,
            );
            assert_eq!(Payable::allowance(&env, owner, spender), 300);
        });
    }
}
