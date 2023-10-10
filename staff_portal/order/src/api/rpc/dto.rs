use std::vec::Vec;

use serde::{Deserialize, Serialize};
use chrono::DateTime;
use chrono::offset::{Local, FixedOffset};

use crate::api::{jsn_validate_product_type, jsn_serialize_product_type };
use crate::constant::ProductType;

// TODO, merge the 2 DTO modules in `/web` and `/rpc` package

#[derive(Deserialize)]
pub struct ProductPriceDeleteDto {
    pub items:Option<Vec<u64>>,
    pub pkgs :Option<Vec<u64>>,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub item_type:ProductType,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub pkg_type:ProductType,
}

#[derive(Deserialize)]
pub struct ProductPriceEditDto {
    pub price: u32,
    pub start_after: DateTime<Local>,
    pub end_before: DateTime<Local>,
    // Note: This order-processing application doesn't need to know the meaning
    // of the field `product type` from this API endpoint, it is just for identifying
    // specific product in specific storefront. There is no need to convert the value
    // at here.
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
}

#[derive(Deserialize)]
pub struct ProductPriceDto {
    pub s_id: u32, // store ID
    pub rm_all: bool,
    pub deleting: ProductPriceDeleteDto,
    pub updating: Vec<ProductPriceEditDto>,
    pub creating: Vec<ProductPriceEditDto>
}

#[derive(Deserialize)]
pub struct InventoryEditStockLevelDto {
    // number to add to stock level, negative number means cancellation
    // from inventory application
    pub qty_add: i32,
    pub store_id: u32,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>
}

#[derive(Serialize)]
pub struct StockQuantityPresentDto {
    pub total: u32,
    pub booked: u32,
    pub cancelled: u32,
}

#[derive(Serialize)]
pub struct StockLevelPresentDto {
    pub quantity: StockQuantityPresentDto,
    pub store_id: u32,
    #[serde(serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>
}
