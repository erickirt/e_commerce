use std::result::Result;

use chrono::{DateTime, Utc};
use mysql_async::Params;
use rust_decimal::Decimal;

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use super::super::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use super::{inner_into_parts, raw_column_to_datetime, DATETIME_FMT_P0F, DATETIME_FMT_P3F};
use crate::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerMetaModel,
    ChargeBuyerModel, ChargeLineBuyerModel, PayLineAmountModel,
};

struct InsertChargeTopLvlArgs(String, Params);
struct InsertChargeStatusArgs {
    curr_state: String,
    t_accepted: Option<String>,
    t_completed: Option<String>,
    t_order_app_synced: Option<String>,
}
struct InsertChargeLinesArgs(String, Vec<Params>);
struct UpdateCharge3partyArgs {
    label: String,
    detail: String,
}
struct UpdateChargeStatusArgs {
    curr_state: String,
    // the 1st element of the tuple incidates column name which saves
    // the stringified time value in the 2nd element of the tuple.
    time_column: (String, String),
}

pub(super) struct InsertChargeArgs(pub(super) Vec<(String, Vec<Params>)>);
pub(super) struct FetchChargeMetaArgs(String, Params);
pub(super) struct UpdateChargeMetaArgs(String, Params);
pub(super) struct FetchChargeLineArgs(String, Params);

pub(super) type ChargeMetaRowType = (
    Vec<u8>,
    String,                     // `state`
    Option<mysql_async::Value>, // `processor_accepted_time`
    Option<mysql_async::Value>, // `processor_completed_time`
    Option<mysql_async::Value>, // `orderapp_synced_time`
    String,                     // `pay_method`
    String,                     // `detail_3rdparty`, serialised json
);

#[rustfmt::skip]
pub(super) type ChargeLineRowType = (
    u32, String, u64, Decimal, Decimal, u32
);

impl TryFrom<BuyerPayInState> for InsertChargeStatusArgs {
    type Error = AppRepoError;
    fn try_from(value: BuyerPayInState) -> Result<Self, Self::Error> {
        let (curr_state, times) = match value {
            BuyerPayInState::Initialized => Err(AppErrorCode::InvalidInput),
            BuyerPayInState::ProcessorAccepted(t) => Ok(("ProcessorAccepted", 0, Some(t))),
            BuyerPayInState::ProcessorCompleted(t) => Ok(("ProcessorCompleted", 1, Some(t))),
            BuyerPayInState::OrderAppSynced(t) => Ok(("OrderAppSynced", 2, Some(t))),
        }
        .map(|(label, idx, option_t)| {
            const REPEAT_INIT_VALUE: Option<String> = None;
            let mut times = [REPEAT_INIT_VALUE; 3];
            if let Some(t) = option_t {
                times[idx] = Some(t.format(DATETIME_FMT_P3F).to_string());
            }
            (label.to_string(), times)
        })
        .map_err(|code| AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateCharge,
            code,
            detail: AppRepoErrorDetail::ChargeStatus(value),
        })?;
        Ok(Self {
            curr_state,
            t_accepted: times[0].to_owned(),
            t_completed: times[1].to_owned(),
            t_order_app_synced: times[2].to_owned(),
        })
    } // end of fn try_from
} // end of impl InsertChargeStatusArgs

impl TryFrom<BuyerPayInState> for UpdateChargeStatusArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: BuyerPayInState) -> Result<Self, Self::Error> {
        let result = match value {
            BuyerPayInState::Initialized => Err(AppRepoErrorDetail::ChargeStatus(value)),
            BuyerPayInState::ProcessorAccepted(t) => {
                Ok(("ProcessorAccepted", "processor_accepted_time", t))
            }
            BuyerPayInState::ProcessorCompleted(t) => {
                Ok(("ProcessorCompleted", "processor_completed_time", t))
            }
            BuyerPayInState::OrderAppSynced(t) => Ok(("OrderAppSynced", "orderapp_synced_time", t)),
        };
        result
            .map_err(|detail| (AppErrorCode::InvalidInput, detail))
            .map(|(state_val, state_t_col, t)| Self {
                curr_state: state_val.to_string(),
                time_column: (
                    state_t_col.to_string(),
                    t.format(DATETIME_FMT_P3F).to_string(),
                ),
            })
    } // end of fn try_from
} // end of impl UpdateChargeStatusArgs

impl TryFrom<Charge3partyModel> for UpdateCharge3partyArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    #[rustfmt::skip]
    fn try_from(value: Charge3partyModel) -> Result<Self, Self::Error> {
        match value {
            Charge3partyModel::Stripe(m) => {
                let label = "Stripe".to_string();
                serde_json::to_string(&m)
                    .map_err(|e| (
                        AppErrorCode::DataCorruption,
                        AppRepoErrorDetail::PayDetail(label.clone(), e.to_string()),
                    ))
                    .map(|detail| Self {label, detail})
            }
            Charge3partyModel::Unknown =>
                Err((
                    AppErrorCode::InvalidInput,
                    AppRepoErrorDetail::PayMethodUnsupport("unknown".to_string()),
                )),
        }
    } // end of fn try-from
} // end of impl UpdateCharge3partyArgs

impl TryFrom<ChargeBuyerModel> for InsertChargeTopLvlArgs {
    type Error = AppRepoError;
    #[rustfmt::skip]
    fn try_from(value: ChargeBuyerModel) -> Result<Self, Self::Error> {
        // at this point the currency snapshot and charge lines should be handled
        // elsewhere, no need to insert again
        let (
            owner, create_time, oid, state, method,
        ) = value.meta.into_parts();
        let oid_b = OidBytes::try_from(oid.as_str()).map_err(|(code, msg)| AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateCharge,
            detail: AppRepoErrorDetail::OrderIDparse(msg),
            code,
        })?;
        let InsertChargeStatusArgs {
            curr_state,
            t_accepted,
            t_completed,
            t_order_app_synced,
        } = InsertChargeStatusArgs::try_from(state)?;
        let UpdateCharge3partyArgs {
            label: pay_mthd,
            detail: detail_3pty,
        } = UpdateCharge3partyArgs::try_from(method).map_err(
            |(code, detail)| AppRepoError {
                fn_label: AppRepoErrorFnLabel::CreateCharge,  detail, code,
            })?;
        let arg = vec![
            owner.into(),
            create_time.format(DATETIME_FMT_P0F).to_string().into(),
            oid_b.0.into(),
            curr_state.into(),
            t_accepted.into(),
            t_completed.into(),
            t_order_app_synced.into(),
            pay_mthd.into(),
            detail_3pty.into(),
        ];
        let params = Params::Positional(arg);
        let stmt = "INSERT INTO `charge_buyer_toplvl`(`usr_id`,`create_time`,`order_id`,\
                    `state`,`processor_accepted_time`,`processor_completed_time`,\
                    `orderapp_synced_time`,`pay_method`,`detail_3rdparty`) VALUES \
                    (?,?,?,?,?,?,?,?,?)";
        Ok(Self(stmt.to_string(), params))
    }
} // end of impl InsertChargeTopLvlArgs

impl From<(u32, String, Vec<ChargeLineBuyerModel>)> for InsertChargeLinesArgs {
    fn from(value: (u32, String, Vec<ChargeLineBuyerModel>)) -> Self {
        let (buyer_id, ctime, lines) = value;
        let params = lines
            .into_iter()
            .map(|line| {
                let (pid, amount_orig, _amount_refunded) = line.into_parts();
                // TODO, add relevant columns, save `amount_refunded` to table
                let BaseProductIdentity {
                    store_id,
                    product_type,
                    product_id,
                } = pid;
                let PayLineAmountModel { unit, total, qty } = amount_orig;
                let prod_type_num: u8 = product_type.into();
                let arg = vec![
                    buyer_id.into(),
                    ctime.as_str().into(),
                    store_id.into(),
                    prod_type_num.to_string().into(),
                    product_id.into(),
                    unit.into(),
                    total.into(),
                    qty.into(),
                ];
                Params::Positional(arg)
            })
            .collect();
        let stmt = "INSERT INTO `charge_line`(`buyer_id`,`create_time`,`store_id`,\
                    `product_type`,`product_id`,`amt_unit`,`amt_total`,`qty`) \
                    VALUES (?,?,?,?,?,?,?,?)";
        Self(stmt.to_string(), params)
    } // end of fn from
} // end of impl InsertChargeLinesArgs

impl TryFrom<ChargeBuyerModel> for InsertChargeArgs {
    type Error = AppRepoError;
    fn try_from(mut value: ChargeBuyerModel) -> Result<Self, Self::Error> {
        // TODO, modify order-line replica if input charge state is already
        // in `completed` state
        let (buyer_id, ctime) = (
            value.meta.owner(),
            value
                .meta
                .create_time()
                .format(DATETIME_FMT_P0F)
                .to_string(),
        );
        let c_lines = value.lines.split_off(0);
        assert!(value.lines.is_empty());
        let toplvl_arg = InsertChargeTopLvlArgs::try_from(value)?;
        let lines_arg = InsertChargeLinesArgs::from((buyer_id, ctime, c_lines));
        let inner = vec![
            (toplvl_arg.0, vec![toplvl_arg.1]),
            (lines_arg.0, lines_arg.1),
        ];
        Ok(Self(inner))
    }
} // end of impl InsertChargeArgs

impl From<(u32, DateTime<Utc>)> for FetchChargeMetaArgs {
    fn from(value: (u32, DateTime<Utc>)) -> Self {
        let stmt = "SELECT `order_id`,`state`,`processor_accepted_time`,`processor_completed_time`,\
                    `orderapp_synced_time`,`pay_method`,`detail_3rdparty` FROM `charge_buyer_toplvl`\
                    WHERE `usr_id`=? AND `create_time`=?";
        let args = vec![
            value.0.into(),
            value.1.format(DATETIME_FMT_P0F).to_string().into(),
        ];
        Self(stmt.to_string(), Params::Positional(args))
    }
}

inner_into_parts!(FetchChargeMetaArgs);

impl TryFrom<(String, [Option<mysql_async::Value>; 3])> for BuyerPayInState {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (String, [Option<mysql_async::Value>; 3])) -> Result<Self, Self::Error> {
        let (label, time_records) = value;
        let mut time_records = time_records.to_vec();
        assert_eq!(time_records.len(), 3);
        let result = match label.as_str() {
            "ProcessorAccepted" => {
                if let Some(t) = time_records.remove(0) {
                    let t = raw_column_to_datetime(t, 3)?;
                    Ok(Self::ProcessorAccepted(t))
                } else {
                    Err("3pty-accepted-missing-time")
                }
            }
            "ProcessorCompleted" => {
                if let Some(t) = time_records.remove(1) {
                    let t = raw_column_to_datetime(t, 3)?;
                    Ok(Self::ProcessorCompleted(t))
                } else {
                    Err("3pty-completed-missing-time")
                }
            }
            "OrderAppSynced" => {
                if let Some(t) = time_records.remove(2) {
                    let t = raw_column_to_datetime(t, 3)?;
                    Ok(Self::OrderAppSynced(t))
                } else {
                    Err("orderapp-synced-missing-time")
                }
            }
            _others => Err("invalid-buy-in-state"),
        };
        result.map_err(|msg| {
            (
                AppErrorCode::DataCorruption,
                AppRepoErrorDetail::DataRowParse(format!("{msg}: {label}")),
            )
        })
    }
} // end of impl BuyerPayInState

impl TryFrom<(String, String)> for Charge3partyModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (String, String)) -> Result<Self, Self::Error> {
        let (label, detail) = value;
        let result = match label.as_str() {
            "Stripe" => serde_json::from_str::<Charge3partyStripeModel>(detail.as_str())
                .map(Charge3partyModel::Stripe)
                .map_err(|e| e.to_string()),
            _others => Err(format!("unknown-3pty-method: {}", label)),
        };
        result.map_err(|msg| {
            (
                AppErrorCode::DataCorruption,
                AppRepoErrorDetail::DataRowParse(msg),
            )
        })
    }
}

#[rustfmt::skip]
impl TryFrom<(u32, DateTime<Utc>, ChargeMetaRowType)> for ChargeBuyerMetaModel {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    fn try_from(value: (u32, DateTime<Utc>, ChargeMetaRowType)) -> Result<Self, Self::Error> {
        let (
            owner, create_time,
            (
                oid_raw,
                buyin_state,
                accepted_time_3pty,
                completed_time_3pty,
                orderapp_synced_time,
                mthd_3pty_label,
                detail_3pty_serial,
            ),
        ) = value;
        let oid = OidBytes::to_app_oid(oid_raw)
            .map_err(|(code, msg)| (code, AppRepoErrorDetail::DataRowParse(msg)))?;
        let state = BuyerPayInState::try_from((
            buyin_state,
            [
                accepted_time_3pty,
                completed_time_3pty,
                orderapp_synced_time,
            ],
        ))?;
        let method = Charge3partyModel::try_from((mthd_3pty_label, detail_3pty_serial))?;
        let mut obj = Self::from((oid, owner, create_time));
        obj.update_progress(&state);
        obj.update_3party(method);
        Ok(obj)
    } // end of fn try-from
} // end of impl ChargeBuyerMetaModel

impl TryFrom<ChargeBuyerMetaModel> for UpdateChargeMetaArgs {
    type Error = (AppErrorCode, AppRepoErrorDetail);
    #[rustfmt::skip]
    fn try_from(value: ChargeBuyerMetaModel) -> Result<Self, Self::Error> {
        let (owner, create_time, _, state, method) = value.into_parts();
        let UpdateCharge3partyArgs {
            label: _,
            detail: detail_3pty,
        } = UpdateCharge3partyArgs::try_from(method)?;
        let UpdateChargeStatusArgs {
            curr_state: state_col,
            time_column: (state_t_col_name, state_t_col_value),
        } = UpdateChargeStatusArgs::try_from(state)?;
        let args = vec![
            detail_3pty.into(),
            state_col.into(),
            state_t_col_value.into(),
            owner.into(),
            create_time.format(DATETIME_FMT_P0F).to_string().into(),
        ];
        let params = Params::Positional(args);
        let stmt = format!(
            "UPDATE `charge_buyer_toplvl` SET `detail_3rdparty`=?, \
            `state`=?, `{state_t_col_name}`=? WHERE `usr_id`=? \
             AND `create_time`=?"
        );
        Ok(Self(stmt, params))
    } // end of fn try-from
} // end of impl UpdateChargeMetaArgs

inner_into_parts!(UpdateChargeMetaArgs);

impl From<(u32, DateTime<Utc>, Option<u32>)> for FetchChargeLineArgs {
    fn from(value: (u32, DateTime<Utc>, Option<u32>)) -> Self {
        let mut stmt = "SELECT `store_id`,`product_type`,`product_id`,`amt_unit`,\
                    `amt_total`,`qty` FROM `charge_line` WHERE `buyer_id`=? \
                    AND `create_time`=?"
            .to_string();
        let mut args = vec![
            value.0.into(),
            value.1.format(DATETIME_FMT_P0F).to_string().into(),
        ];
        if let Some(store_id) = value.2 {
            stmt += " AND `store_id`=?";
            args.push(store_id.into());
        }
        Self(stmt, Params::Positional(args))
    }
}
impl FetchChargeLineArgs {
    pub(super) fn into_parts(self) -> (String, Params) {
        let Self(stmt, params) = self;
        (stmt, params)
    }
}

impl TryFrom<ChargeLineRowType> for ChargeLineBuyerModel {
    type Error = AppRepoErrorDetail;
    #[rustfmt::skip]
    fn try_from(value: ChargeLineRowType) -> Result<Self, Self::Error> {
        let (
            store_id, product_type_serial, product_id,
            amt_unit, amt_total, qty,
        ) = value;
        let product_type = product_type_serial.parse::<ProductType>()
            .map_err(|e| AppRepoErrorDetail::DataRowParse(e.0.to_string()))?;
        let pid = BaseProductIdentity { store_id, product_id, product_type };
        let amount_orig = PayLineAmountModel {
            unit: amt_unit, total: amt_total, qty,
        };
        // TODO, add relevant table columns, load it from DB, convert to amount-refunded field
        let amount_refunded = PayLineAmountModel::default();
        let out = Self::from((pid, amount_orig, amount_refunded));
        Ok(out)
    }
}
