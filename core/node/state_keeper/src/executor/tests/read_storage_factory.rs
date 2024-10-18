use anyhow::Context;
use async_trait::async_trait;
use zksync_concurrency::{ctx,error::Wrap as _};
use zksync_dal::{ConnectionPool, Core};
use zksync_state::{OwnedStorage, RocksdbStorage};
use zksync_types::L1BatchNumber;
use crate::state_keeper_storage::ReadStorageFactory;

#[derive(Debug, Clone)]
pub struct RocksdbStorageFactory {
    pool: ConnectionPool<Core>,
    state_keeper_db_path: String,
}

#[async_trait]
impl ReadStorageFactory for RocksdbStorageFactory {
    async fn access_storage(
        &self,
        ctx: &ctx::Ctx,
        _l1_batch_number: L1BatchNumber,
    ) -> ctx::Result<OwnedStorage> {
        let builder = RocksdbStorage::builder(self.state_keeper_db_path.as_ref())
            .await
            .context("Failed opening state keeper RocksDB")?;
        let mut conn = ctx.wait(self
            .pool
            .connection_tagged("state_keeper"))
            .await?
            .context("Failed getting a connection to Postgres")?;
        let rocksdb_storage = builder
            .synchronize(ctx, &mut conn, None)
            .await
            .wrap("Failed synchronizing state keeper's RocksDB to Postgres")?;
        Ok(OwnedStorage::Rocksdb(rocksdb_storage))
    }
}

impl RocksdbStorageFactory {
    pub fn new(pool: ConnectionPool<Core>, state_keeper_db_path: String) -> Self {
        Self {
            pool,
            state_keeper_db_path,
        }
    }
}
