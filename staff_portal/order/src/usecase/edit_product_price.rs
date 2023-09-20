use std::result::Result as DefaultResult;

use crate::AppSharedState;
use crate::error::AppError;
use crate::logging::{app_log_event, AppLogLevel};
use crate::api::rpc::dto::ProductPriceDto;
use crate::repository::app_repo_product_price;

pub struct EditProductPriceUseCase {}

impl EditProductPriceUseCase {
    pub async fn execute(app_state:AppSharedState, data:ProductPriceDto)
        -> DefaultResult<(), AppError>
    {
        let ds = app_state.datastore();
        let repo = app_repo_product_price(ds)?;
        let result = if data.rm_all {
            repo.delete_all(data.s_id).await
        } else if data.deleting.items.is_some() || data.deleting.pkgs.is_some() {
            // currently the storefront service separates delete operation from
            // create and update operations, we can expect there is no product overlapped
            // in the `deleting`, `creating`, and `updating` lists
            repo.delete(data.s_id, data.deleting).await
        } else { // create and update
            let ids = data.updating.iter().map(
                |d| (d.product_type, d.product_id)).collect();
            match repo.fetch(data.s_id, ids).await {
                Ok(previous_saved) => {
                    let updated = previous_saved.update(data.updating, data.creating);
                    repo.save(updated).await
                }, Err(e) => Err(e)
            }
        };
        if let Err(e) = &result {
            let logctx = app_state.log_context().clone();
            app_log_event!(logctx, AppLogLevel::ERROR, "detail:{}", e);    
        }
        result
    } // end of fn execute
} // end of impl EditProductPriceUseCase