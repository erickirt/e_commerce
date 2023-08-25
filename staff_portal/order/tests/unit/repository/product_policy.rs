use std::sync::Arc;
use std::boxed::Box;

use order::{AppDataStoreContext, AppInMemoryDbCfg};
use order::error::{AppErrorCode, AppError};
use order::datastore::{AbstInMemoryDStore, AppInMemoryDStore, AppInMemUpdateData, AppInMemFetchKeys, AppInMemFetchedData, AppInMemDeleteInfo} ;
use order::repository::{ProductPolicyInMemRepo, AbstProductPolicyRepo};
use order::model::{ProductPolicyModelSet, ProductPolicyModel};
use crate::ut_clone_productpolicy_model;

const UTEST_INIT_DATA: [ProductPolicyModel;7] = [
    ProductPolicyModel {
        usr_id: 123, product_id: 1556, auto_cancel_secs: 309,
        warranty_hours: 7400, async_stock_chk: true, is_create: true
    },
    ProductPolicyModel {
        usr_id: 124, product_id: 9273, auto_cancel_secs: 900,
        warranty_hours: 7209, async_stock_chk: false, is_create: true
    },
    ProductPolicyModel {
        usr_id: 123, product_id: 40051, auto_cancel_secs: 707,
        warranty_hours: 1295, async_stock_chk: true, is_create: true
    },
    ProductPolicyModel {
        usr_id: 124, product_id: 1620, auto_cancel_secs: 1645,
        warranty_hours: 1918, async_stock_chk: false, is_create: true
    },
    ProductPolicyModel {
        usr_id: 123, product_id: 14005, auto_cancel_secs: 77,
        warranty_hours: 5129, async_stock_chk: true, is_create: true
    },
    ProductPolicyModel {
        usr_id: 124, product_id: 1622, auto_cancel_secs: 6451,
        warranty_hours: 9181, async_stock_chk: false, is_create: true
    },
    ProductPolicyModel {
        usr_id: 124, product_id: 1622, auto_cancel_secs: 1178,
        warranty_hours: 11086, async_stock_chk: true, is_create: false
    },
]; // end of UTEST_INIT_DATA

#[test]
fn in_mem_create_missing_dstore ()
{
    let ds_ctx = Arc::new(AppDataStoreContext{in_mem:None, sql_dbs:None});
    let result = ProductPolicyInMemRepo::new(ds_ctx);
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::MissingDataStore);
    assert_eq!(error.detail, Some("in-memory".to_string()));
}

fn in_mem_repo_ds_setup<T: AbstInMemoryDStore + 'static> ()
    -> Box<dyn AbstProductPolicyRepo>
{
    let ds_ctx = {
        let d = AppInMemoryDbCfg { alias:format!("utest") , max_items:20 };
        let obj = T::new(&d);
        let obj:Box<dyn AbstInMemoryDStore> = Box::new(obj);
        let inmem_ds = Arc::new(obj);
        Arc::new(AppDataStoreContext{ sql_dbs:None,
            in_mem:Some(inmem_ds) })
    };
    let result = ProductPolicyInMemRepo::new(ds_ctx);
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

#[tokio::test]
async fn in_mem_save_fetch_ok_1 ()
{
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>();
    // ------ subcase, first bulk update
    let ppset = {
        let items = UTEST_INIT_DATA[0..3].iter().map(ut_clone_productpolicy_model).collect();
        ProductPolicyModelSet { policies: items }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);
    let chosen_usr_id = 123;
    let chosen_ids = vec![14005, 1556, 40051];
    let result = repo.fetch(chosen_usr_id, chosen_ids).await;
    {
        assert_eq!(result.is_ok(), true);
        let modelset = result.unwrap();
        assert_eq!(modelset.policies.len(), 2);
        let exists = modelset.policies.iter().find_map(
            |m| {if m.product_id == 1556 {Some(m)} else {None}}  );
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[0]);
        let exists = modelset.policies.iter().find_map(
            |m| {if m.product_id == 40051 {Some(m)} else {None}}  );
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[2]);
        let exists = modelset.policies.iter().any(|m| {m.product_id == 14005});
        assert_eq!(exists, false);
    }
    // ------ subcase, second bulk update
    let ppset = {
        let items = UTEST_INIT_DATA[3..6].iter().map(ut_clone_productpolicy_model).collect();
        ProductPolicyModelSet { policies: items }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);
    let chosen_usr_id = 124;
    let chosen_ids = vec![1622, 1620, 9273];
    let result = repo.fetch(chosen_usr_id, chosen_ids).await;
    {
        let modelset = result.unwrap();
        let exists = modelset.policies.iter().find_map(
            |m| {if m.product_id == 9273 {Some(m)} else {None}}  );
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[1]);
        let exists = modelset.policies.iter().find_map(
            |m| {if m.product_id == 1620 {Some(m)} else {None}}  );
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[3]);
        let exists = modelset.policies.iter().find_map(
            |m| {if m.product_id == 1622 {Some(m)} else {None}}  );
        assert_eq!(exists.unwrap(), &UTEST_INIT_DATA[5]);
    }
} // end of fn in_mem_save_fetch_ok_1


#[tokio::test]
async fn in_mem_save_fetch_ok_2 ()
{
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>();
    let ppset = {
        let item = ut_clone_productpolicy_model(&UTEST_INIT_DATA[5]);
        ProductPolicyModelSet { policies: vec![item] }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);
    let ppset = {
        let item = ut_clone_productpolicy_model(&UTEST_INIT_DATA[6]);
        ProductPolicyModelSet { policies: vec![item] }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_ok(), true);

    let result = repo.fetch(124u32, vec![1622u64]).await;
    {
        assert_eq!(result.is_ok(), true);
        let modelset = result.unwrap();
        let fetched = modelset.policies.iter().find_map(
            |m| {if m.product_id == 1622 {Some(m)} else {None}}
        ).unwrap();
        assert_eq!(fetched, &UTEST_INIT_DATA[6]);
        assert_ne!(fetched, &UTEST_INIT_DATA[5]);
    }
} // end of fn in_mem_save_fetch_ok_1


#[tokio::test]
async fn in_mem_save_empty_input ()
{
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>();
    let ppset = ProductPolicyModelSet { policies: Vec::new() };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::EmptyInputData);
}


struct MockInMemDStore {}

impl AbstInMemoryDStore for MockInMemDStore {
    fn new(_cfg:&AppInMemoryDbCfg) -> Self where Self:Sized
    { Self{} }
    fn fetch(&self, _info: AppInMemFetchKeys) -> Result<AppInMemFetchedData, AppError> {
        Err(AppError { code: AppErrorCode::AcquireLockFailure, detail:Some(format!("utest")) }) 
    }
    fn delete(&self, _info:AppInMemDeleteInfo) -> Result<usize, AppError> {
        Err(AppError { code: AppErrorCode::NotImplemented, detail:Some(format!("utest")) })
    }
    fn create_table (&self, _label:&str) -> Result<(), AppError> {
        Ok(())
    }
    fn save(&self, _data:AppInMemUpdateData) -> Result<usize, AppError> {
        Err(AppError { code: AppErrorCode::DataTableNotExist, detail:Some(format!("utest")) })
    }
}


#[tokio::test]
async fn in_mem_save_dstore_error ()
{
    let repo = in_mem_repo_ds_setup::<MockInMemDStore>();
    let ppset = {
        let item = ut_clone_productpolicy_model(&UTEST_INIT_DATA[0]);
        ProductPolicyModelSet { policies: vec![item] }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::DataTableNotExist);
    assert_eq!(error.detail, Some("utest".to_string()));
} // end of in_mem_save_dstore_error


#[tokio::test]
async fn in_mem_fetch_dstore_error ()
{
    let repo = in_mem_repo_ds_setup::<MockInMemDStore>();
    let result = repo.fetch(124u32, vec![1622u64]).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::AcquireLockFailure);
    assert_eq!(error.detail, Some("utest".to_string()));
}
