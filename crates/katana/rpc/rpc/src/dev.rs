use std::str::FromStr;
use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor};
use katana_executor::ExecutorFactory;
use katana_primitives::genesis::constant::{
    get_fee_token_balance_base_storage_address, ERC20_NAME_STORAGE_SLOT,
};
use katana_primitives::ContractAddress;
use katana_provider::traits::state::StateFactoryProvider;
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_types::account::Account;
use katana_rpc_types::error::dev::DevApiError;
use starknet_crypto::Felt;

#[allow(missing_debug_implementations)]
pub struct DevApi<EF: ExecutorFactory> {
    backend: Arc<Backend<EF>>,
    block_producer: BlockProducer<EF>,
}

impl<EF: ExecutorFactory> DevApi<EF> {
    pub fn new(backend: Arc<Backend<EF>>, block_producer: BlockProducer<EF>) -> Self {
        Self { backend, block_producer }
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    fn pending_executor(&self) -> Option<PendingExecutor> {
        match &*self.block_producer.producer.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.executor()),
        }
    }

    fn has_pending_transactions(&self) -> bool {
        if let Some(ref exec) = self.pending_executor() {
            !exec.read().transactions().is_empty()
        } else {
            false
        }
    }

    pub fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), DevApiError> {
        if self.has_pending_transactions() {
            return Err(DevApiError::PendingTransactions);
        }

        let mut block_context_generator = self.backend.block_context_generator.write();
        block_context_generator.next_block_start_time = timestamp;

        Ok(())
    }

    pub fn increase_next_block_timestamp(&self, offset: u64) -> Result<(), DevApiError> {
        if self.has_pending_transactions() {
            return Err(DevApiError::PendingTransactions);
        }

        let mut block_context_generator = self.backend.block_context_generator.write();
        block_context_generator.block_timestamp_offset += offset as i64;
        Ok(())
    }
}

#[async_trait]
impl<EF: ExecutorFactory> DevApiServer for DevApi<EF> {
    async fn generate_block(&self) -> Result<(), Error> {
        self.block_producer.force_mine();
        Ok(())
    }

    async fn next_block_timestamp(&self) -> Result<(), Error> {
        // Ok(self.sequencer.backend().env.read().block.block_timestamp.0)
        Ok(())
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        Ok(self.set_next_block_timestamp(timestamp)?)
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        Ok(self.increase_next_block_timestamp(timestamp)?)
    }

    async fn set_storage_at(
        &self,
        _contract_address: Felt,
        _key: Felt,
        _value: Felt,
    ) -> Result<(), Error> {
        // self.sequencer
        //     .set_storage_at(contract_address.into(), key, value)
        //     .await
        //     .map_err(|_| Error::from(KatanaApiError::FailedToUpdateStorage))
        Ok(())
    }

    async fn account_balance(&self, address: String) -> Result<u128, Error> {
        let account_address: ContractAddress = Felt::from_str(&address).unwrap().into();
        let provider = self.backend.blockchain.provider();
        let state = provider.latest().unwrap();
        // let storage_slot =
        //     get_storage_var_address("ERC20_balances", &[account_address.into()]).unwrap();
        let storage_slot = get_fee_token_balance_base_storage_address(account_address);
        let balance_felt = state
            .storage(self.backend.chain_spec.fee_contracts.eth, storage_slot)
            .unwrap()
            .unwrap();
        let balance: u128 = balance_felt.to_string().parse().unwrap();
        Ok(balance)
    }

    async fn fee_token(&self) -> Result<String, Error> {
        let provider = self.backend.blockchain.provider();
        let state = provider.latest().unwrap();
        let fee_token = state
            .storage(self.backend.chain_spec.fee_contracts.eth, ERC20_NAME_STORAGE_SLOT)
            .unwrap()
            .unwrap();
        Ok(fee_token.to_string())
    }

    async fn mint(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error> {
        Ok(self.backend.chain_spec.genesis.accounts().map(|e| Account::new(*e.0, e.1)).collect())
    }
}
