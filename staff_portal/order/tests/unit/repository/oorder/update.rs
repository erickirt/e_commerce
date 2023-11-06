use chrono::{DateTime, FixedOffset};
use order::api::rpc::dto::{OrderLinePaidUpdateDto, OrderPaymentUpdateDto, OrderLinePayUpdateErrorDto, OrderLinePayUpdateErrorReason};
use order::constant::ProductType;
use order::repository::{AbsOrderRepo, OrderInMemRepo};
use order::model::OrderLineModel;

use super::{in_mem_repo_ds_setup, ut_setup_billing, ut_setup_shipping, ut_setup_orderlines};

async fn ut_setup_saved_order() -> (OrderInMemRepo, String)
{
    let o_repo = in_mem_repo_ds_setup(30).await;
    let (mock_usr_id, mock_seller_ids) = (124, [17u32,38]);
    let mock_oid = OrderLineModel::generate_order_id(7);
    let orderlines = ut_setup_orderlines(&mock_seller_ids);
    let mut billings = ut_setup_billing();
    let mut shippings = ut_setup_shipping(&mock_seller_ids);
    let result = o_repo.create(mock_oid.clone(), mock_usr_id,  orderlines,
                               billings.remove(0), shippings.remove(0)).await;
    assert!(result.is_ok());
    (o_repo, mock_oid)
}

fn ut_setup_oline_new_payment() -> Vec<OrderLinePaidUpdateDto>
{
    let paid_time = [
        "2023-11-17T09:23:50+05:00", "2023-11-16T11:49:00+05:00",
        "2023-11-16T18:09:51+08:00"
    ].into_iter().map(|s| {
        DateTime::parse_from_rfc3339(s).unwrap()
    }).collect::<Vec<DateTime<FixedOffset>>>();
    vec![
        OrderLinePaidUpdateDto { seller_id: 38, product_type:ProductType::Item,
            product_id: 190, qty: 1, time:paid_time[0] },
        OrderLinePaidUpdateDto { seller_id: 17, product_type:ProductType::Item,
            product_id: 190, qty: 1, time: paid_time[1] },
        OrderLinePaidUpdateDto { seller_id: 17, product_type:ProductType::Package,
            product_id: 190, qty: 2, time: paid_time[2] }
    ]
}

fn ut_usr_cb_ok(models:&mut Vec<OrderLineModel>, data:Vec<OrderLinePaidUpdateDto>)
    -> Vec<OrderLinePayUpdateErrorDto>
{
    assert_eq!(models.len(), 3);
    data.into_iter().map(|d| {
        let result = models.iter_mut().find(
            |m| (m.seller_id==d.seller_id && m.product_id==d.product_id
                 && m.product_type==d.product_type )
        );
        assert!(result.is_some());
        let saved = result.unwrap();
        assert_eq!(saved.qty.paid, 0);
        assert!(saved.qty.paid_last_update.is_none());
        saved.qty.paid = d.qty;
        saved.qty.paid_last_update = Some(d.time);
    }).count();
    vec![]
}

#[tokio::test]
async fn in_mem_update_lines_payment_ok()
{
    let (o_repo, oid) = ut_setup_saved_order().await;
    let lines = ut_setup_oline_new_payment();
    let data = OrderPaymentUpdateDto {oid:oid.clone(), lines};
    let result = o_repo.update_lines_payment(data, ut_usr_cb_ok).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.oid, oid);
        assert_eq!(v.lines.len(), 0);
    }
    //TODO, reload order lines
} // end of fn in_mem_update_lines_payment_ok


fn ut_usr_cb_err(models:&mut Vec<OrderLineModel>, data:Vec<OrderLinePaidUpdateDto>)
    -> Vec<OrderLinePayUpdateErrorDto>
{
    assert_eq!(models.len(), 3);
    vec![
        OrderLinePayUpdateErrorDto {seller_id: 38, product_type:ProductType::Item,
            product_id: 190, reason: OrderLinePayUpdateErrorReason::ReservationExpired },
        OrderLinePayUpdateErrorDto {seller_id: 17, product_type:ProductType::Package,
            product_id: 190, reason: OrderLinePayUpdateErrorReason::InvalidQuantity },
        OrderLinePayUpdateErrorDto {seller_id: 17, product_type:ProductType::Item,
            product_id: 190, reason: OrderLinePayUpdateErrorReason::Omitted },
    ]
}

#[tokio::test]
async fn in_mem_update_lines_payment_usr_cb_err()
{
    let (o_repo, oid) = ut_setup_saved_order().await;
    let mut lines = ut_setup_oline_new_payment();
    lines[1].qty  = 9999;
    lines[2].time = DateTime::parse_from_rfc3339("1999-07-31T23:59:59+09:00").unwrap();
    let data = OrderPaymentUpdateDto {oid:oid.clone(), lines};
    let result = o_repo.update_lines_payment(data, ut_usr_cb_err).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.oid, oid);
        assert_eq!(v.lines.len(), 3);
    }
}


#[tokio::test]
async fn in_mem_update_lines_payment_dstore_err()
{
}
