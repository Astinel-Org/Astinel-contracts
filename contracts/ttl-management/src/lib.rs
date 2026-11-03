#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec,
};

#[contracttype]
#[derive(Clone)]
pub struct DataEntry {
    pub key: Symbol,
    pub value: Bytes,
    pub ttl_ledgers: u32,
    pub created_at: u32,
}

const ENTRIES: Symbol = symbol_short!("ENTRIES");
const ADMIN: Symbol = symbol_short!("ADMIN");
const LIVE_LEDGERS: u32 = 500_000;

#[contract]
pub struct TtlManagement;

#[contractimpl]
impl TtlManagement {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&ADMIN, &admin);
        let empty: Vec<Symbol> = Vec::new(&env);
        env.storage().instance().set(&ENTRIES, &empty);
    }

    fn only_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        caller.require_auth();
        assert_eq!(caller.clone(), admin, "TTL: caller is not admin");
    }

    pub fn write(env: Env, caller: Address, key: Symbol, value: Bytes, ttl_ledgers: u32) {
        Self::only_admin(&env, &caller);
        assert!(ttl_ledgers > 0, "TTL: ttl_ledgers must be positive");

        let entry = DataEntry {
            key: key.clone(),
            value,
            ttl_ledgers,
            created_at: env.ledger().sequence(),
        };

        env.storage().persistent().set(&key, &entry);
        env.storage()
            .persistent()
            .extend_ttl(&key, ttl_ledgers, ttl_ledgers + LIVE_LEDGERS);

        let mut entries: Vec<Symbol> = env.storage().instance().get(&ENTRIES).unwrap();
        entries.push_back(key);
        env.storage().instance().set(&ENTRIES, &entries);
    }

    pub fn read(env: &Env, key: Symbol) -> Option<DataEntry> {
        env.storage().persistent().get(&key)
    }

    pub fn remove(env: Env, caller: Address, key: Symbol) {
        Self::only_admin(&env, &caller);
        env.storage().persistent().remove(&key);
        let entries: Vec<Symbol> = env.storage().instance().get(&ENTRIES).unwrap();
        let mut filtered: Vec<Symbol> = Vec::new(&env);
        for k in entries.iter() {
            if k != key {
                filtered.push_back(k);
            }
        }
        env.storage().instance().set(&ENTRIES, &filtered);
    }

    pub fn extend_ttl(env: Env, caller: Address, key: Symbol, ttl_ledgers: u32) {
        Self::only_admin(&env, &caller);
        assert!(ttl_ledgers > 0, "TTL: ttl_ledgers must be positive");
        let mut entry: DataEntry = env.storage().persistent().get(&key).unwrap();
        entry.ttl_ledgers = ttl_ledgers;
        // Reset created_at so remaining_ttl stays consistent with the new TTL window
        entry.created_at = env.ledger().sequence();
        env.storage().persistent().set(&key, &entry);
        env.storage()
            .persistent()
            .extend_ttl(&key, ttl_ledgers, ttl_ledgers + LIVE_LEDGERS);
    }

    pub fn entries(env: &Env) -> Vec<Symbol> {
        env.storage().instance().get(&ENTRIES).unwrap()
    }

    /// Returns the remaining TTL ledgers for the entry identified by `key`.
    ///
    /// The on-ledger TTL threshold after a write/extend call is
    /// `ttl_ledgers + LIVE_LEDGERS` (the `max_ttl` arg of `extend_ttl`).
    /// We account for the `LIVE_LEDGERS` buffer so the reported value matches
    /// what the Soroban host actually maintains on-ledger.
    pub fn remaining_ttl(env: &Env, key: Symbol) -> Option<u32> {
        let entry: DataEntry = env.storage().persistent().get(&key)?;
        let elapsed = env.ledger().sequence().saturating_sub(entry.created_at);
        // Include the LIVE_LEDGERS bump so the value matches on-ledger TTL
        let total_ttl = entry.ttl_ledgers.saturating_add(LIVE_LEDGERS);
        Some(total_ttl.saturating_sub(elapsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn create_env() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(TtlManagement, ());
        (env, contract_id)
    }

    #[test]
    fn test_write_and_read() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            TtlManagement::init(env.clone(), admin.clone());

            let key = symbol_short!("DATA1");
            let value = Bytes::from_array(&env, &[1, 2, 3, 4]);
            TtlManagement::write(
                env.clone(),
                admin.clone(),
                key.clone(),
                value.clone(),
                100_000,
            );

            let entry = TtlManagement::read(&env, key.clone()).unwrap();
            assert_eq!(entry.value, value);
        });
    }

    #[test]
    fn test_remove() {
        let (env, contract_id) = create_env();
        env.mock_all_auths_allowing_non_root_auth();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let key = symbol_short!("DATA1");
            TtlManagement::init(env.clone(), admin.clone());

            let value = Bytes::from_array(&env, &[1, 2, 3]);
            TtlManagement::write(env.clone(), admin.clone(), key.clone(), value, 100_000);

            // Change admin to allow a fresh auth frame for remove
            let admin2 = Address::generate(&env);
            env.storage().instance().set(&ADMIN, &admin2);
            TtlManagement::remove(env.clone(), admin2.clone(), key.clone());
            assert!(TtlManagement::read(&env, key).is_none());
        });
    }

    #[test]
    fn test_remaining_ttl_accounts_for_live_ledgers_buffer() {
        let (env, contract_id) = create_env();
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            TtlManagement::init(env.clone(), admin.clone());

            let key = symbol_short!("TKEY");
            let value = Bytes::from_array(&env, &[9, 8, 7]);
            let ttl_ledgers: u32 = 100_000;

            let seq_before = env.ledger().sequence();
            TtlManagement::write(env.clone(), admin.clone(), key.clone(), value, ttl_ledgers);

            // remaining_ttl must equal (ttl_ledgers + LIVE_LEDGERS) - elapsed
            let elapsed = env.ledger().sequence().saturating_sub(seq_before);
            let expected = ttl_ledgers.saturating_add(LIVE_LEDGERS).saturating_sub(elapsed);
            let actual = TtlManagement::remaining_ttl(&env, key).unwrap();

            assert_eq!(
                actual, expected,
                "remaining_ttl must account for LIVE_LEDGERS buffer: expected {expected}, got {actual}"
            );
        });
    }

    #[test]
    fn test_remaining_ttl_none_for_missing_key() {
        let (env, contract_id) = create_env();
        env.as_contract(&contract_id, || {
            TtlManagement::init(env.clone(), Address::generate(&env));
            assert!(TtlManagement::remaining_ttl(&env, symbol_short!("NOPE")).is_none());
        });
    }
}
