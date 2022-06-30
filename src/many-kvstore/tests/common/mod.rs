use many::{
    server::module::{
        idstore::{CredentialId},
    },
    types::{
        identity::{cose::testsutils::generate_random_eddsa_identity},
    },
    Identity, 
};
use many_kvstore::module::{KvStoreModuleImpl};

pub struct Setup {
    pub module_impl: KvStoreModuleImpl,
    pub id: Identity,
    pub cred_id: CredentialId,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Setup {
    pub fn new(blockchain: bool) -> Self {
        let id = generate_random_eddsa_identity();

        let content = std::fs::read_to_string("../../staging/kvstore_state.json").unwrap();
        let state = serde_json::from_str(&content).unwrap();
        
        Self {
            module_impl: KvStoreModuleImpl::new(
                state,
                tempfile::tempdir().unwrap(),
                blockchain
            ).unwrap(),
            id: id.identity,
            cred_id: CredentialId(vec![1; 16].into()),
        }
    }
}

pub fn setup() -> Setup {
    Setup::default()
}
