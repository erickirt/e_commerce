use std::boxed::Box;
use std::sync::Arc;

use ecommerce_common::api::rpc::dto::{OrderReplicaPaymentDto, OrderReplicaPaymentReqDto};
use ecommerce_common::api::web::dto::BillingErrorDto;
use ecommerce_common::model::order::BillingModel;

use crate::adapter::cache::{AbstractOrderSyncLockCache, OrderSyncLockError};
use crate::adapter::processor::{
    AbstractPaymentProcessor, AppProcessorError, AppProcessorPayInResult,
};
use crate::adapter::repository::{AbstractChargeRepo, AppRepoError};
use crate::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};
use crate::api::web::dto::{
    ChargeAmountOlineDto, ChargeReqDto, ChargeRespDto, ChargeRespErrorDto, PaymentMethodErrorReason,
};
use crate::model::{ChargeLineModelSet, OLineModelError, OrderLineModelSet};

// TODO, switch to enum type then add memberis `SessionCreated`,
// `PayInDone` when the charge can be done in one single API call
pub struct ChargeCreateUcResult(ChargeRespDto);

pub enum ChargeCreateUcError {
    OrderOwnerMismatch,                   // client error, e.g. status code 403
    ClientBadRequest(ChargeRespErrorDto), // status code 400
    OrderNotExist,
    LockCacheError,
    LoadOrderConflict, // client error, e.g. status code 429
    LoadOrderInternalError(AppRpcCtxError),
    LoadOrderByteCorruption(String),
    RpcBillingParseError(BillingErrorDto),
    RpcOlineParseError(Vec<OLineModelError>),
    ExternalProcessorError(PaymentMethodErrorReason),
    DataStoreError(AppRepoError),
}

impl From<OrderSyncLockError> for ChargeCreateUcError {
    fn from(_value: OrderSyncLockError) -> Self {
        Self::LockCacheError
    }
}
impl From<AppRpcCtxError> for ChargeCreateUcError {
    fn from(value: AppRpcCtxError) -> Self {
        Self::LoadOrderInternalError(value)
    }
}
impl From<AppRepoError> for ChargeCreateUcError {
    fn from(value: AppRepoError) -> Self {
        Self::DataStoreError(value)
    }
}
impl From<AppProcessorError> for ChargeCreateUcError {
    fn from(value: AppProcessorError) -> Self {
        Self::ExternalProcessorError(value.reason)
    }
}
impl From<serde_json::Error> for ChargeCreateUcError {
    fn from(value: serde_json::Error) -> Self {
        Self::LoadOrderByteCorruption(value.to_string())
    }
}
impl From<BillingErrorDto> for ChargeCreateUcError {
    fn from(value: BillingErrorDto) -> Self {
        Self::RpcBillingParseError(value)
    }
}
impl From<Vec<OLineModelError>> for ChargeCreateUcError {
    fn from(value: Vec<OLineModelError>) -> Self {
        Self::RpcOlineParseError(value)
    }
}
impl From<ChargeRespErrorDto> for ChargeCreateUcError {
    fn from(value: ChargeRespErrorDto) -> Self {
        Self::ClientBadRequest(value)
    }
}

pub struct ChargeCreateUseCase {
    pub processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    pub rpc_ctx: Arc<Box<dyn AbstractRpcContext>>,
    pub ordersync_lockset: Arc<Box<dyn AbstractOrderSyncLockCache>>,
    pub repo: Box<dyn AbstractChargeRepo>,
}

impl ChargeCreateUseCase {
    pub async fn execute(
        &self,
        usr_id: u32,
        req_body: ChargeReqDto,
    ) -> Result<ChargeCreateUcResult, ChargeCreateUcError> {
        let oid = req_body.order_id.as_str();
        let result = self.try_load_order(usr_id, oid, &req_body.lines).await?;
        let validated_order = if let Some(v) = result {
            v
        } else {
            let d = self.rpc_sync_order(usr_id, oid).await?;
            self.try_save_order(usr_id, oid, d, &req_body.lines).await?
        };
        let (cline_set, payin_result) = self
            .try_execute_processor(validated_order, req_body)
            .await?;
        if payin_result.completed {
            // TODO, if the pay-in process is complete, invoke RPC to order service
            // for payment status update
        }
        let resp = ChargeRespDto::from(cline_set);
        Ok(ChargeCreateUcResult(resp))
    } // end of fn execute

    async fn try_load_order(
        &self,
        usr_id_uncheck: u32,
        oid_uncheck: &str,
        lines_uncheck: &[ChargeAmountOlineDto],
    ) -> Result<Option<OrderLineModelSet>, ChargeCreateUcError> {
        let result = self
            .repo
            .get_unpaid_olines(usr_id_uncheck, oid_uncheck)
            .await?;
        if let Some(saved) = result.as_ref() {
            // TODO, internal data store error, should log message
            ChargeLineModelSet::validate(saved, lines_uncheck)?;
        }
        Ok(result)
    }

    async fn rpc_sync_order(
        &self,
        usr_id: u32,
        oid: &str,
    ) -> Result<OrderReplicaPaymentDto, ChargeCreateUcError> {
        let success = self.ordersync_lockset.acquire(usr_id, oid).await?;
        if success {
            let out = self._rpc_sync_order(oid).await;
            self.ordersync_lockset.release(usr_id, oid).await?;
            out
        } else {
            Err(ChargeCreateUcError::LoadOrderConflict)
        }
    }
    async fn _rpc_sync_order(
        &self,
        oid: &str,
    ) -> Result<OrderReplicaPaymentDto, ChargeCreateUcError> {
        let client = self.rpc_ctx.acquire().await?;
        let payld = OrderReplicaPaymentReqDto {
            order_id: oid.to_string(),
        };
        let props = AppRpcClientRequest {
            message: serde_json::to_vec(&payld).unwrap(),
            route: "rpc.order.order_reserved_replica_payment".to_string(),
        };
        let mut event = client.send_request(props).await?;
        let reply = event.receive_response().await?;
        let out = serde_json::from_slice::<OrderReplicaPaymentDto>(&reply.message)?;
        Ok(out)
    }

    async fn try_save_order(
        &self,
        usr_id_uncheck: u32,
        oid_uncheck: &str,
        rpc_data: OrderReplicaPaymentDto,
        lines_uncheck: &[ChargeAmountOlineDto],
    ) -> Result<OrderLineModelSet, ChargeCreateUcError> {
        let OrderReplicaPaymentDto {
            oid,
            usr_id,
            lines,
            billing,
        } = rpc_data;
        let billing = BillingModel::try_from(billing)?;
        let olines = OrderLineModelSet::try_from((oid, usr_id, lines))?;
        self.repo.create_order(&olines, &billing).await?;
        let mismatch = (olines.id.as_str() != oid_uncheck) || (olines.owner != usr_id_uncheck);
        if mismatch {
            Err(ChargeCreateUcError::OrderOwnerMismatch)
        } else {
            ChargeLineModelSet::validate(&olines, lines_uncheck)?;
            Ok(olines)
        }
    }

    async fn try_execute_processor(
        &self,
        order: OrderLineModelSet,
        reqbody: ChargeReqDto,
    ) -> Result<(ChargeLineModelSet, AppProcessorPayInResult), ChargeCreateUcError> {
        let cline_set = ChargeLineModelSet::from((order, reqbody));
        let result = self.processors.pay_in_start(&cline_set).await?;
        self.repo.create_charge(&cline_set).await?;
        Ok((cline_set, result))
    }
} // end of impl ChargeCreateUseCase