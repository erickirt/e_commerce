#![feature(io_error_more)]
use std::sync::Arc;

pub mod api;
pub mod error;
pub mod logging;
pub mod network;
pub mod constant;
pub mod usecase;
pub mod repository;
pub mod model;
pub mod confidentiality;

mod config;
pub use config::{
    AppConfig, ApiServerCfg, WebApiListenCfg, WebApiRouteCfg, AppLoggingCfg,
    AppLogHandlerCfg, AppLoggerCfg, AppBasepathCfg, AppRpcCfg, AppRpcAmqpCfg,
    AppInMemoryDbCfg, AppConfidentialCfg, AppAuthCfg
};

mod auth;
pub use auth::{
    AbstractAuthKeystore, AppAuthKeystore, AppJwtAuthentication, AppKeystoreRefreshResult,
    AppAuthedClaim, AppAuthClaimQuota, AppAuthClaimPermission
};

mod rpc;
use rpc::build_context as build_rpc_context;
pub use rpc::{AbstractRpcContext, AbsRpcServerCtx, AbsRpcClientCtx,  AbstractRpcClient,
    AbstractRpcServer, AppRpcReply, AppRpcClientReqProperty
};

mod adapter;
pub use adapter::datastore;

use confidentiality::AbstractConfidentiality;

type WebApiPath = String;
type WebApiHdlrLabel = & 'static str;
type AppLogAlias = Arc<String>;

pub struct AppDataStoreContext {
    pub in_mem: Option<Arc<Box<dyn datastore::AbstInMemoryDStore>>>,
    pub sql_dbs: Option<Vec<Arc<datastore::AppMariaDbStore>>>
} // TODO, rename sql_dbs

// global state shared by all threads
pub struct AppSharedState {
    _cfg: Arc<AppConfig>,
    _log: Arc<logging::AppLogContext>,
    _rpc: Arc<Box<dyn AbstractRpcContext>>,
    dstore: Arc<AppDataStoreContext>,
    _auth_keys: Arc<Box<dyn AbstractAuthKeystore>>,
}

impl AppSharedState {
    pub fn new(cfg:AppConfig, log:logging::AppLogContext, confidential:Box<dyn AbstractConfidentiality>) -> Self
    { // TODO, confidential argument to arc-box pointer
        let confidential = Arc::new(confidential);
        let log = Arc::new(log);
        let _rpc_ctx = build_rpc_context(&cfg.api_server.rpc, confidential.clone());
        let (in_mem, sql_dbs) = datastore::build_context(log.clone(),
                                &cfg.api_server.data_store, confidential);
        let in_mem = if let Some(m) = in_mem { Some(Arc::new(m)) } else {None};
        let sql_dbs = if let Some(m) = sql_dbs {
            Some(m.into_iter().map(Arc::new).collect())
        } else {None};
        let ds_ctx = Arc::new(AppDataStoreContext {in_mem, sql_dbs});
        let auth_keys = AppAuthKeystore::new(&cfg.api_server.auth);
        Self{_cfg:Arc::new(cfg), _log:log, _rpc:Arc::new(_rpc_ctx),
             dstore: ds_ctx, _auth_keys: Arc::new(Box::new(auth_keys)) }
    } // end of fn new

    pub fn config(&self) -> &Arc<AppConfig>
    { &self._cfg }

    pub fn log_context(&self) -> &Arc<logging::AppLogContext>
    { &self._log }
    
    pub fn rpc(&self) -> Arc<Box<dyn AbstractRpcContext>>
    { self._rpc.clone() }

    pub fn datastore(&self) -> Arc<AppDataStoreContext>
    { self.dstore.clone() }

    pub fn auth_keystore(&self) -> Arc<Box<dyn AbstractAuthKeystore>>
    { self._auth_keys.clone() }
} // end of impl AppSharedState

impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self{
            _cfg: self._cfg.clone(),   _log: self._log.clone(),
            _rpc: self._rpc.clone(),   dstore: self.dstore.clone(),
            _auth_keys: self._auth_keys.clone(),
        }
    }
}