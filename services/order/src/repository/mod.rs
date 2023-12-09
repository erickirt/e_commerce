mod product_policy;
mod product_price;
mod stock_level;
mod order;
mod oline_return;

use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::AppDataStoreContext;
use crate::api::dto::OrderLinePayDto;
use crate::api::rpc::dto::{
    ProductPriceDeleteDto, OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto,
    OrderLinePayUpdateErrorDto, OrderLinePaidUpdateDto, StockLevelReturnDto, StockReturnErrorDto
};
use crate::api::web::dto::OrderLineCreateErrorDto;
use crate::constant::ProductType;
use crate::error::AppError;
use crate::model::{
    ProductPolicyModelSet, ProductPriceModelSet, StockLevelModelSet, ProductStockIdentity,
    BillingModel, OrderLineModel, OrderLineModelSet, ShippingModel, OrderLineIdentity, OrderReturnModel
};

// make it visible only for testing purpose
pub use self::order::OrderInMemRepo;
pub use self::oline_return::OrderReturnInMemRepo;
pub use self::product_policy::ProductPolicyInMemRepo;
pub use self::product_price::ProductPriceInMemRepo;
use self::stock_level::StockLvlInMemRepo;

// the repository instance may be used across an await,
// the future created by app callers has to be able to pass to different threads
// , it is the reason to add `Send` and `Sync` as super-traits
#[async_trait]
pub trait AbstProductPolicyRepo : Sync + Send
{
    async fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError>
        where Self:Sized ;
    
    async fn fetch(&self, ids:Vec<(ProductType, u64)>) -> DefaultResult<ProductPolicyModelSet, AppError>;
    
    async fn save(&self, ppset:ProductPolicyModelSet) -> DefaultResult<(), AppError>;
    // TODO, delete operation
}

#[async_trait]
pub trait AbsProductPriceRepo : Sync + Send
{
    async fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
        where Self:Sized ;
    async fn delete_all(&self, store_id:u32) -> DefaultResult<(), AppError>;
    async fn delete(&self, store_id:u32, ids:ProductPriceDeleteDto) -> DefaultResult<(), AppError> ;
    async fn fetch(&self, store_id:u32, ids:Vec<(ProductType,u64)>) -> DefaultResult<ProductPriceModelSet, AppError> ;
    // fetch prices of products from different sellers  at a time, the
    // first element of the `ids` tuple should be valid seller ID
    async fn fetch_many(&self, ids:Vec<(u32,ProductType,u64)>) -> DefaultResult<Vec<ProductPriceModelSet>, AppError> ;
    async fn save(&self, updated:ProductPriceModelSet) -> DefaultResult<(), AppError> ;
}


#[async_trait]
pub trait AbsOrderRepo : Sync + Send {
    async fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized;

    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>;
    
    async fn create (&self, lines:OrderLineModelSet, bl:BillingModel, sh:ShippingModel)
        -> DefaultResult<Vec<OrderLinePayDto>, AppError> ;

    async fn fetch_all_lines(&self, oid:String) -> DefaultResult<Vec<OrderLineModel>, AppError>;

    async fn fetch_billing(&self, oid:String) -> DefaultResult<BillingModel, AppError>;
    
    async fn fetch_shipping(&self, oid:String) -> DefaultResult<ShippingModel, AppError>;
    
    async fn update_lines_payment(&self, data:OrderPaymentUpdateDto,
                                  cb:AppOrderRepoUpdateLinesUserFunc)
        -> DefaultResult<OrderPaymentUpdateErrorDto, AppError>;

    async fn fetch_lines_by_rsvtime(&self, time_start: DateTime<FixedOffset>,
                                  time_end: DateTime<FixedOffset>,
                                  usr_cb: AppOrderFetchRangeCallback )
        -> DefaultResult<(), AppError>;
        
    async fn fetch_lines_by_pid(&self, oid:&str, pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderLineModel>, AppError>;

    async fn fetch_ids_by_created_time(&self,  start: DateTime<FixedOffset>,
                                       end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<String>, AppError>;

    async fn owner_id(&self, order_id:&str) -> DefaultResult<u32, AppError>;
    async fn created_time(&self, order_id:&str) -> DefaultResult<DateTime<FixedOffset>, AppError>;

    // TODO, rename to `cancel_unpaid_last_time()` and `cancel_unpaid_time_update()`
    async fn scheduled_job_last_time(&self) -> DateTime<FixedOffset>;
    async fn scheduled_job_time_update(&self);
} // end of trait AbsOrderRepo

pub type AppOrderRepoUpdateLinesUserFunc = fn(&mut Vec<OrderLineModel>, Vec<OrderLinePaidUpdateDto>)
    -> Vec<OrderLinePayUpdateErrorDto>;

// declare a callback function type which can easily be passed,
// - I made the return type to be `Future` trait object wrapped in `Pin` type
//   because `Future` (generated by async block expression) does not implement `Unpin` trait,
//   that means the `Future`  bobject cannot be moved to different memory locations once
//   generated.
// - the placeholder lifetime `'_` specified in the `Future` trait object will elide
//   lifetime check in this module, not sure how Rust compiler processes this under the
//   hood, but it looks like the lifetime check will be done in given / external callback
//   function signature
pub type AppOrderFetchRangeCallback = fn(&dyn AbsOrderRepo, OrderLineModelSet)
    -> Pin<Box<dyn Future<Output=DefaultResult<(),AppError>> + Send + '_>>;

pub type AppStockRepoReserveReturn = DefaultResult<(), DefaultResult<Vec<OrderLineCreateErrorDto>, AppError>>;

pub type AppStockRepoReserveUserFunc = fn(&mut StockLevelModelSet, &OrderLineModelSet)
    -> AppStockRepoReserveReturn;

// if the function pointer type is declared directly in function signature of a
// trait method, the function pointer will be viewed as closure block
pub type AppStockRepoReturnUserFunc = fn(&mut StockLevelModelSet, StockLevelReturnDto)
    -> Vec<StockReturnErrorDto>;

#[async_trait]
pub trait AbsOrderStockRepo : Sync + Send {
    async fn fetch(&self, pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>;
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>;
    async fn try_reserve(&self, cb: AppStockRepoReserveUserFunc,
                         order_req: &OrderLineModelSet) -> AppStockRepoReserveReturn;
    async fn try_return(&self,  cb: AppStockRepoReturnUserFunc,
                        data: StockLevelReturnDto )
        -> DefaultResult<Vec<StockReturnErrorDto>, AppError>;
}


#[async_trait]
pub trait AbsOrderReturnRepo : Sync + Send {
    async fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderReturnRepo>, AppError>
        where Self: Sized;
    async fn fetch_by_pid(&self, oid:&str, pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>; 
    async fn fetch_by_created_time(&self, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError>;
    async fn fetch_by_oid_ctime(&self, oid:&str, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>;
    async fn save(&self, oid:&str, reqs:Vec<OrderReturnModel>) -> DefaultResult<usize, AppError>;
}


// TODO, consider runtime configuration for following repositories

pub async fn app_repo_product_policy (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbstProductPolicyRepo>, AppError>
{
    ProductPolicyInMemRepo::new(ds).await
}
pub async fn app_repo_product_price (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
{
    ProductPriceInMemRepo::new(ds).await
}
pub async fn app_repo_order (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
{
    OrderInMemRepo::new(ds).await
}
pub async fn app_repo_order_return (ds:Arc<AppDataStoreContext>)
    -> DefaultResult<Box<dyn AbsOrderReturnRepo>, AppError>
{
    OrderReturnInMemRepo::new(ds).await
}
