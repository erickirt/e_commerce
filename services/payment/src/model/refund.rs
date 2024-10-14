use std::cmp::min;
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use ecommerce_common::constant::ProductType;
use rust_decimal::Decimal;

use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::model::BaseProductIdentity;

use super::{
    ChargeBuyerModel, ChargeLineBuyerMap, ChargeLineBuyerModel, OrderCurrencySnapshot,
    PayLineAmountError, PayLineAmountModel,
};
use crate::api::web::dto::{
    RefundCompletionOlineReqDto, RefundCompletionReqDto, RefundCompletionRespDto,
    RefundLineRejectDto, RefundRejectReasonDto,
};

#[derive(Debug)]
pub enum RefundErrorParseOline {
    CreateTime(String),
    Amount(PayLineAmountError),
}
#[derive(Debug)]
pub enum RefundModelError {
    ParseOline {
        pid: BaseProductIdentity,
        reason: RefundErrorParseOline,
    },
    QtyInsufficient {
        pid: BaseProductIdentity,
        num_avail: u32,
        num_req: u32,
    },
    AmountInsufficient {
        pid: BaseProductIdentity,
        num_avail: Decimal,
        num_req: Decimal,
    },
    MissingReqLine(BaseProductIdentity, DateTime<Utc>),
    MissingCurrency(String, u32),
} // end of enum RefundModelError

// quantities of product items rejected to refund for defined reasons
pub struct RefundLineQtyRejectModel(RefundLineRejectDto);

pub struct RefundLineResolveAmountModel {
    // accumulated qty / amount against single line
    accumulated: PayLineAmountModel,
    curr_round: PayLineAmountModel,
}
struct RefundLineReqResolutionModel {
    pid: BaseProductIdentity,
    time_req: DateTime<Utc>,
    qty_reject: RefundLineQtyRejectModel,
    // the amount should be present in buyer's currency
    amount: RefundLineResolveAmountModel,
}

pub struct RefundReqResolutionModel {
    buyer_usr_id: u32,
    charged_ctime: DateTime<Utc>,
    currency_buyer: OrderCurrencySnapshot,
    currency_merc: OrderCurrencySnapshot,
    lines: Vec<RefundLineReqResolutionModel>,
}

pub struct OLineRefundModel {
    pid: BaseProductIdentity,
    amount_req: PayLineAmountModel,
    // the time when customer issued the refund request,
    time_req: DateTime<Utc>,
    // keep `resolution` history data along with each line
    amount_refunded: PayLineAmountModel,
    rejected: RefundLineQtyRejectModel,
    // TODO, reconsider whether or not to add each time the merchant
    // finalized the refund request line, to provide more detail log
}

pub struct OrderRefundModel {
    id: String, // order-id
    lines: Vec<OLineRefundModel>,
}

#[rustfmt::skip]
impl RefundModelError {
    fn qty_limit(pid: &BaseProductIdentity, num_avail:u32, num_req:u32) -> Self {
        Self::QtyInsufficient { pid: pid.clone(), num_avail, num_req }
    }
    fn amount_limit(
        pid: &BaseProductIdentity, num_avail: Decimal, num_req: Decimal
    ) -> Self {
        Self::AmountInsufficient {
            pid: pid.clone(), num_avail, num_req
        }
    }
} // end of impl RefundModelError

impl<'a> From<&'a RefundLineRejectDto> for RefundLineQtyRejectModel {
    fn from(value: &'a RefundLineRejectDto) -> Self {
        Self(value.clone())
    }
}
impl Default for RefundLineQtyRejectModel {
    fn default() -> Self {
        let iter = [
            RefundRejectReasonDto::Damaged,
            RefundRejectReasonDto::Fraudulent,
        ]
        .into_iter()
        .map(|k| (k, 0u32));
        let inner = HashMap::from_iter(iter);
        Self(inner)
    }
}
impl RefundLineQtyRejectModel {
    fn total_qty(&self) -> u32 {
        self.0.values().sum()
    }
}

impl RefundLineResolveAmountModel {
    pub fn curr_round(&self) -> &PayLineAmountModel {
        &self.curr_round
    }
    pub fn accumulated(&self) -> &PayLineAmountModel {
        &self.accumulated
    }
}

#[rustfmt::skip]
impl<'a> From<(&'a PayLineAmountModel, u32, Decimal)> for RefundLineResolveAmountModel {
    fn from(value: (&'a PayLineAmountModel, u32, Decimal)) -> Self {
        let (prev_rfd, qty, amt_tot) = value;
        let accumulated = PayLineAmountModel {
            unit: prev_rfd.unit, total: prev_rfd.total, qty: prev_rfd.qty
        };
        let curr_round = PayLineAmountModel {
            unit: prev_rfd.unit, total: amt_tot, qty
        };
        Self { accumulated, curr_round }
    }
}

#[rustfmt::skip]
impl TryFrom<OrderLineReplicaRefundDto> for OLineRefundModel {
    type Error = RefundModelError;
    
    fn try_from(value: OrderLineReplicaRefundDto) -> Result<Self, Self::Error> {
        let OrderLineReplicaRefundDto {
            seller_id, product_id, product_type, create_time, amount, qty
        } = value;
        let pid = BaseProductIdentity { store_id: seller_id, product_type, product_id };
        let time_req = DateTime::parse_from_rfc3339(create_time.as_str())
            .map_err(|e| RefundModelError::ParseOline {
                pid: pid.clone(),
                reason: RefundErrorParseOline::CreateTime(e.to_string())
            })?.to_utc();
        let unit = Decimal::from_str(amount.unit.as_str())
            .map_err(|e| RefundModelError::ParseOline {
                pid: pid.clone(),
                reason: RefundErrorParseOline::Amount(
                    PayLineAmountError::ParseUnit(amount.unit, e.to_string())
                )
            })?;
        let total = Decimal::from_str(amount.total.as_str())
            .map_err(|e| RefundModelError::ParseOline {
                pid: pid.clone(),
                reason: RefundErrorParseOline::Amount(
                    PayLineAmountError::ParseTotal(amount.total, e.to_string())
                )
            })?;
        let amount_req = PayLineAmountModel { unit, total, qty };
        let amount_refunded = PayLineAmountModel::default();
        let rejected = RefundLineQtyRejectModel::default();
        Ok(Self { pid, amount_req, time_req, amount_refunded, rejected })
    } // end of fn try-from
} // end of impl OLineRefundModel

type OLineRefundCvtArgs = (
    BaseProductIdentity,
    PayLineAmountModel,
    DateTime<Utc>,
    PayLineAmountModel,
    RefundLineQtyRejectModel,
);

impl From<OLineRefundCvtArgs> for OLineRefundModel {
    #[rustfmt::skip]
    fn from(value: OLineRefundCvtArgs) -> Self {
        let (pid, amount_req, time_req, amount_refunded, rejected) = value;
        Self { pid, amount_req, time_req, amount_refunded, rejected }
    }
}

impl OLineRefundModel {
    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> OLineRefundCvtArgs {
        let Self { pid, amount_req, time_req, amount_refunded, rejected } = self;
        (pid, amount_req, time_req, amount_refunded, rejected)
    }

    fn estimate_remain_quantity(
        &self,
        data: &RefundCompletionOlineReqDto,
    ) -> Result<u32, RefundModelError> {
        let detail = (
            self.amount_req.qty,
            self.amount_refunded.qty,
            self.rejected.total_qty(),
        );
        let qty_avail = detail
            .0
            .checked_sub(detail.1)
            .ok_or(RefundModelError::qty_limit(&self.pid, detail.0, detail.1))?;
        let qty_avail = qty_avail
            .checked_sub(detail.2)
            .ok_or(RefundModelError::qty_limit(&self.pid, qty_avail, detail.2))?;
        let detail = (qty_avail, data.approval.quantity, data.total_qty_rejected());
        let qty_avail = detail
            .0
            .checked_sub(detail.1)
            .ok_or(RefundModelError::qty_limit(&self.pid, detail.0, detail.1))?;
        let qty_avail = qty_avail
            .checked_sub(detail.2)
            .ok_or(RefundModelError::qty_limit(&self.pid, qty_avail, detail.2))?;
        Ok(qty_avail)
    }

    fn estimate_remain_amount(
        &self,
        data: &RefundCompletionOlineReqDto,
    ) -> Result<Decimal, RefundModelError> {
        let amt_new_aprv = Decimal::from_str(data.approval.amount_total.as_str()).map_err(|e| {
            RefundModelError::ParseOline {
                pid: self.pid.clone(),
                reason: RefundErrorParseOline::Amount(PayLineAmountError::ParseTotal(
                    data.approval.amount_total.clone(),
                    e.to_string(),
                )),
            }
        })?;
        let qty_discard = data.total_qty_rejected();
        let detail = (
            self.amount_req.total,
            self.amount_refunded.total,
            amt_new_aprv,
            Decimal::new(qty_discard as i64, 0) * self.amount_req.unit,
        );
        macro_rules! check_subtract_amount {
            ($n0: expr, $n1: expr) => {{
                let out = $n0
                    .checked_sub($n1)
                    .ok_or(RefundModelError::amount_limit(&self.pid, $n0, $n1))?;
                if out.is_sign_negative() {
                    return Err(RefundModelError::amount_limit(&self.pid, $n0, $n1));
                }
                out
            }};
        }
        let amt_avail = check_subtract_amount!(detail.0, detail.1);
        let amt_avail = check_subtract_amount!(amt_avail, detail.2);
        let amt_avail = check_subtract_amount!(amt_avail, detail.3);
        Ok(amt_avail)
    } // end of fn estimate_remain_amount

    fn estimate_remains(
        &self,
        data: &RefundCompletionOlineReqDto,
    ) -> Result<(u32, Decimal), RefundModelError> {
        let qty = self.estimate_remain_quantity(data)?;
        let amt_tot = self.estimate_remain_amount(data)?;
        Ok((qty, amt_tot))
    }
} // end of impl OLineRefundModel

impl TryFrom<(String, Vec<OrderLineReplicaRefundDto>)> for OrderRefundModel {
    type Error = Vec<RefundModelError>;

    fn try_from(value: (String, Vec<OrderLineReplicaRefundDto>)) -> Result<Self, Self::Error> {
        let (oid, d_lines) = value;
        let mut errs = Vec::new();
        let lines = d_lines
            .into_iter()
            .filter_map(|d| OLineRefundModel::try_from(d).map_err(|e| errs.push(e)).ok())
            .collect::<Vec<_>>();
        if errs.is_empty() {
            Ok(Self { id: oid, lines })
        } else {
            Err(errs)
        }
    }
} // end of impl OrderRefundModel

impl From<(String, Vec<OLineRefundModel>)> for OrderRefundModel {
    fn from(value: (String, Vec<OLineRefundModel>)) -> Self {
        let (oid, lines) = value;
        Self { id: oid, lines }
    }
}

impl OrderRefundModel {
    pub(crate) fn into_parts(self) -> (String, Vec<OLineRefundModel>) {
        let Self { id: oid, lines } = self;
        (oid, lines)
    }
    pub(crate) fn num_lines(&self) -> usize {
        self.lines.len()
    }
    pub(crate) fn merchant_ids(&self) -> Vec<u32> {
        let iter = self.lines.iter().map(|v| v.pid.store_id);
        let hset: HashSet<u32, RandomState> = HashSet::from_iter(iter);
        hset.into_iter().collect()
    }

    pub fn validate(
        &self,
        merchant_id: u32,
        data: &RefundCompletionReqDto,
    ) -> Result<Vec<(ProductType, u64, DateTime<Utc>, u32, Decimal)>, Vec<RefundModelError>> {
        let mut errors = Vec::new();
        let valid_amt_qty = data
            .lines
            .iter()
            .filter_map(|d| {
                let key = BaseProductIdentity {
                    store_id: merchant_id,
                    product_type: d.product_type.clone(),
                    product_id: d.product_id,
                };
                let result = self
                    .lines
                    .iter()
                    .find(|v| v.pid == key && v.time_req == d.time_issued);
                if let Some(line) = result {
                    match line.estimate_remains(d) {
                        Err(e) => {
                            errors.push(e);
                            None
                        }
                        Ok((qty, amt_tot)) => Some((
                            key.product_type,
                            key.product_id,
                            d.time_issued,
                            qty,
                            amt_tot,
                        )),
                    }
                } else {
                    let e = RefundModelError::MissingReqLine(key, d.time_issued);
                    errors.push(e);
                    None
                }
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(valid_amt_qty)
        } else {
            Err(errors)
        }
    } // end of fn validate

    pub(crate) fn update(&mut self, _rslv_m: &RefundReqResolutionModel) {}
} // end of impl OrderRefundModel

impl RefundLineReqResolutionModel {
    fn to_vec<'a, 'b>(
        c: &'a ChargeLineBuyerModel,
        cmplt_req: &'b RefundCompletionReqDto,
    ) -> Vec<Self> {
        let amt_prev_refunded = c.amount_refunded();
        let mut amt_remain = c.amount_remain();
        cmplt_req
            .lines
            .iter()
            .filter(|r| r.product_id == c.pid.product_id && r.product_type == c.pid.product_type)
            .map(|r| {
                let amt_tot_req = Decimal::from_str(r.approval.amount_total.as_str()).unwrap();
                let qty_fetched = min(amt_remain.qty, r.approval.quantity);
                let tot_amt_fetched = min(amt_remain.total, amt_tot_req);
                if qty_fetched > 0 {
                    amt_remain.qty -= qty_fetched;
                    amt_remain.total -= tot_amt_fetched;
                }
                let arg = (amt_prev_refunded, qty_fetched, tot_amt_fetched);
                Self {
                    pid: c.pid.clone(),
                    time_req: r.time_issued,
                    qty_reject: RefundLineQtyRejectModel::from(&r.reject),
                    amount: RefundLineResolveAmountModel::from(arg),
                }
            })
            .filter(|m| m.total_qty_curr_round() > 0)
            .collect::<Vec<_>>()
    } // end of fn to-vec

    fn total_qty_curr_round(&self) -> u32 {
        let num_rej = self.qty_reject.total_qty();
        let num_aprv = self.amount.curr_round().qty;
        num_rej + num_aprv
    }
} // end of impl RefundLineReqResolutionModel

impl<'a, 'b> TryFrom<(u32, &'a ChargeBuyerModel, &'b RefundCompletionReqDto)>
    for RefundReqResolutionModel
{
    type Error = RefundModelError;
    fn try_from(
        value: (u32, &'a ChargeBuyerModel, &'b RefundCompletionReqDto),
    ) -> Result<Self, Self::Error> {
        let (merchant_id, charge_m, cmplt_req) = value;
        let buyer_usr_id = charge_m.meta.owner();
        let currency_b = charge_m
            .get_buyer_currency()
            .ok_or(RefundModelError::MissingCurrency(
                "buyer-id".to_string(),
                buyer_usr_id,
            ))?;
        let currency_m =
            charge_m
                .get_seller_currency(merchant_id)
                .ok_or(RefundModelError::MissingCurrency(
                    "merchant-id".to_string(),
                    merchant_id,
                ))?;
        let lines = charge_m
            .lines
            .iter()
            .filter(|c| c.pid.store_id == merchant_id)
            .map(|c| RefundLineReqResolutionModel::to_vec(c, cmplt_req))
            .flatten()
            .collect::<Vec<_>>();
        Ok(Self {
            buyer_usr_id,
            charged_ctime: *charge_m.meta.create_time(),
            currency_buyer: currency_b,
            currency_merc: currency_m,
            lines,
        })
    }
} // end of impl RefundReqResolutionModel

impl RefundReqResolutionModel {
    pub(crate) fn update_req(&self, _cmplt_req: &mut RefundCompletionReqDto) {}

    pub(crate) fn to_chargeline_map(_reqs: &[Self]) -> ChargeLineBuyerMap {
        HashMap::new()
    }
    #[rustfmt::skip]
    pub fn get_status(
        &self, merchant_id: u32, product_type: ProductType,
        product_id: u64, time_req: DateTime<Utc>,
    ) -> Option<(&RefundLineRejectDto, &RefundLineResolveAmountModel)> {
        let key = BaseProductIdentity {
            store_id: merchant_id ,product_type,product_id
        };
        self.lines.iter()
            .find(|v| v.pid == key && time_req == v.time_req)
            .map(|v| (&v.qty_reject.0, &v.amount))
    }
} // end of impl RefundReqResolutionModel

impl From<Vec<RefundReqResolutionModel>> for RefundCompletionRespDto {
    fn from(_value: Vec<RefundReqResolutionModel>) -> Self {
        Self { lines: Vec::new() }
    } // TODO, finish implementation
} // end of fn RefundCompletionRespDto
