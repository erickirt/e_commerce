use std::result::Result;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::{CountryCode, CurrencyDto};
use ecommerce_common::api::rpc::dto::{ShopLocationRepDto, StoreProfileReplicaDto};

use super::AppProcessorErrorReason;
use crate::api::web::dto::{StoreOnboardStripeReqDto, StripeCheckoutUImodeDto};
use crate::model::{
    Charge3partyStripeModel, ChargeLineBuyerModel, Payout3partyStripeModel, PayoutInnerModel,
    RefundReqRslvInnerModel, StripeAccountCapabilityModel, StripeAccountCapableState,
    StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};

#[derive(Deserialize)]
pub(super) struct CheckoutSession {
    pub id: String,
    pub client_secret: Option<String>,
    pub url: Option<String>,
    pub status: StripeSessionStatusModel,
    pub payment_status: StripeCheckoutPaymentStatusModel,
    pub payment_intent: String,
    pub expires_at: i64,
    // TODO, record more fields for payout at later time
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CheckoutSessionMode {
    Payment,
    // currently not support other options : Setup, Subscription,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CheckoutSessionUiMode {
    Embedded,
    Hosted,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionPaymentIntentData {
    // `application_fee_amount` only supported in direct charge and payment charge,
    // in this application I use separate charge, the application fee will be charged
    // by reducing amount of payout to relevant  sellers
    pub transfer_group: Option<String>, // for seperate charges
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionProductData {
    pub name: String,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionPriceData {
    pub product_data: CreateCheckoutSessionProductData,
    pub currency: CurrencyDto,
    pub unit_amount_decimal: String,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionLineItem {
    pub price_data: CreateCheckoutSessionPriceData,
    pub quantity: u32,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSession {
    pub client_reference_id: String, // usr-profile-id followed by order-id
    pub currency: CurrencyDto,
    pub customer: Option<String>, // customer-id only, expandable object not supported
    pub expires_at: i64,          // epoch time in seconds at which the checkout will expire
    pub cancel_url: Option<String>,
    pub success_url: Option<String>,
    pub return_url: Option<String>, // for return / refund, TODO, verify
    // TODO, implement Price / Product objects, it is useless for this e-commerce
    // project but essential for Stripe platform
    pub line_items: Vec<CreateCheckoutSessionLineItem>,
    pub payment_intent_data: CreateCheckoutSessionPaymentIntentData,
    pub mode: CheckoutSessionMode,
    pub ui_mode: CheckoutSessionUiMode,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize)]
pub(super) enum ConnectAccountType {
    express, // TODO, support `custom` account type
}

#[derive(Serialize)]
pub(super) struct CreateConnectAccount {
    #[serde(rename = "type")]
    pub type_: ConnectAccountType,
    pub country: CountryCode,
    pub email: String,
    capabilities: AccountCapabilityRequest,
    pub business_profile: AccountBusinessProfile,
    pub tos_acceptance: AccountToSAccept,
    pub settings: AccountSettings,
}

#[derive(Deserialize)]
pub(super) struct ConnectAccount {
    pub id: String,
    // TODO, remove this lint when account type enum has more variant
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub type_: ConnectAccountType,
    pub country: CountryCode,
    pub email: Option<String>,
    pub capabilities: StripeAccountCapabilityModel,
    pub requirements: AccountRequirement,
    pub tos_acceptance: AccountToSAccept,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub details_submitted: bool,
    pub created: i64, // UNIX timestamp
    pub settings: AccountSettings,
}

#[derive(Serialize)]
struct AccountCapabilityRequest {
    // note `card_payments` cannot be supported in many countries
    transfers: AccountCapabilityReqFlag,
}
#[derive(Serialize)]
struct AccountCapabilityReqFlag {
    requested: bool,
}

#[derive(Serialize)]
pub(super) struct AccountBusinessAddress {
    pub country: CountryCode,
    pub city: String,
    pub line1: String,
    pub line2: String,
    // TODO, add fields `state` and `postal_code`
}

#[derive(Serialize)]
pub(super) struct AccountBusinessProfile {
    pub name: String,
    pub support_address: AccountBusinessAddress,
    pub support_email: String,
    pub support_phone: String,
}

#[derive(Deserialize)]
pub(super) struct AccountRequirement {
    pub currently_due: Vec<String>,
    pub disabled_reason: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct AccountToSAccept {
    pub date: Option<i64>,
    pub service_agreement: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct AccountPayoutSchedule {
    pub delay_days: i16,
    pub interval: String,
}
#[derive(Serialize, Deserialize)]
pub(super) struct AccountSettingPayout {
    pub schedule: AccountPayoutSchedule,
    pub debit_negative_balances: bool,
}
#[derive(Serialize, Deserialize)]
pub(super) struct AccountSettings {
    pub payouts: AccountSettingPayout,
}

#[allow(non_camel_case_types)]
#[derive(Serialize)]
pub(super) enum AccountLinkType {
    account_onboarding,
}

#[derive(Serialize)]
pub(super) struct CreateAccountLink {
    pub refresh_url: String,
    pub return_url: String,
    pub account: String,
    #[serde(rename = "type")]
    pub type_: AccountLinkType,
}

#[derive(Deserialize)]
pub(super) struct AccountLink {
    // pub created: i64, // TODO, add `created` back, currently this field is unused
    pub expires_at: i64, // UNIX timestamp
    pub url: String,
}

#[derive(Serialize)]
pub(super) struct CreateTransfer {
    pub currency: CurrencyDto,
    pub destination: String,
    pub amount: i64,
    /// [CAUTION]
    /// DO NOT rely on `transfer_group` field for limiting amount to pay out to
    /// any specific merchant, Stripe platform does not really validate that for
    /// applications consuming the `create-transfer` API endpoint.
    ///
    /// This means anyone can try calling the `create-transfer` API endpoint with
    /// any arbitrary `transfer_group` field and valid currency (used in default
    /// bank account of your Stripe platform),  the Stripe API server still performs
    /// payout operation  even with non-existent `transfer_group`
    pub transfer_group: String,
}

#[derive(Deserialize)]
pub(super) struct Transfer {
    pub id: String,
    // FIXME. serde fails to rename `currency` to uppercase before de-serialization
    // pub currency: CurrencyDto,
    pub destination: String,
    pub amount: i64,
    pub transfer_group: String,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Serialize)]
pub(super) enum RefundReason {
    requested_by_customer,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug)]
pub(super) enum RefundStatus {
    pending,
    requires_action,
    succeeded,
    failed,
    canceled,
}

#[derive(Serialize)]
pub(super) struct CreateRefund {
    payment_intent: String,
    amount: String,
    reason: RefundReason,
}

#[derive(Deserialize)]
pub(super) struct RefundResult {
    id: String,
    #[allow(dead_code)]
    amount: i64,
    #[allow(dead_code)]
    reason: RefundReason,
    // FIXME. serde fails to rename `currency` to uppercase before de-serialization
    // pub currency: CurrencyDto,
    payment_intent: String,
    status: RefundStatus,
}

impl From<&StripeCheckoutUImodeDto> for CheckoutSessionUiMode {
    fn from(value: &StripeCheckoutUImodeDto) -> Self {
        match value {
            StripeCheckoutUImodeDto::EmbeddedJs => Self::Embedded,
            StripeCheckoutUImodeDto::RedirectPage => Self::Hosted,
        }
    }
}

impl CreateCheckoutSessionPriceData {
    fn new(cline: &ChargeLineBuyerModel, currency_label: CurrencyDto) -> Self {
        let amt_unit_represent = Charge3partyStripeModel::amount_represent(
            cline.amount_orig().unit,
            currency_label.clone(),
        )
        .unwrap(); // TODO, overflow error handling
        CreateCheckoutSessionPriceData {
            product_data: CreateCheckoutSessionProductData {
                name: format!("{:?}", cline.id()),
            }, // TODO, load product name, save the product ID in metadata
            currency: currency_label,
            // the unit-amount field has to contain smallest unit
            // of specific currency
            unit_amount_decimal: amt_unit_represent.to_string(),
        }
    }
} // end of impl CreateCheckoutSessionPriceData

impl From<(CurrencyDto, &ChargeLineBuyerModel)> for CreateCheckoutSessionLineItem {
    fn from(value: (CurrencyDto, &ChargeLineBuyerModel)) -> Self {
        let (currency_label, cline) = value;
        let quantity = cline.amount_orig().qty;
        let price_data = CreateCheckoutSessionPriceData::new(cline, currency_label);
        Self {
            price_data,
            quantity,
        }
    }
}

impl Default for ConnectAccountType {
    fn default() -> Self {
        Self::express
    }
}

impl Default for AccountCapabilityRequest {
    #[rustfmt::skip]
    fn default() -> Self {
        let transfers = AccountCapabilityReqFlag { requested: true };
        Self { transfers }
    }
}

impl From<ShopLocationRepDto> for AccountBusinessAddress {
    #[rustfmt::skip]
    fn from(value: ShopLocationRepDto) -> Self {
        let ShopLocationRepDto {
            country, locality: city,
            street, detail: line2, floor,
        } = value;
        let line1 = format!("{street}, {floor}F");
        Self { country, city, line1, line2 }
    }
}

impl From<([String; 3], ShopLocationRepDto)> for AccountBusinessProfile {
    #[rustfmt::skip]
    fn from(value: ([String; 3], ShopLocationRepDto)) -> Self {
        let ([name, support_email, support_phone], location) = value;
        let support_address = AccountBusinessAddress::from(location);
        Self { name, support_address, support_email, support_phone }
    }
}

impl Default for AccountToSAccept {
    fn default() -> Self {
        let service_agreement = "recipient".to_string();
        Self {
            date: None,
            service_agreement,
        }
    }
}

impl Default for AccountPayoutSchedule {
    fn default() -> Self {
        let interval = "daily".to_string();
        Self {
            delay_days: 7,
            interval,
        }
    }
}
impl Default for AccountSettingPayout {
    fn default() -> Self {
        let schedule = AccountPayoutSchedule::default();
        Self {
            schedule,
            debit_negative_balances: false,
        }
    }
}
impl Default for AccountSettings {
    fn default() -> Self {
        let payouts = AccountSettingPayout::default();
        Self { payouts }
    }
}

impl TryFrom<StoreProfileReplicaDto> for CreateConnectAccount {
    type Error = AppProcessorErrorReason;
    fn try_from(value: StoreProfileReplicaDto) -> Result<Self, Self::Error> {
        let mut err_detail: Vec<&str> = Vec::new();
        let StoreProfileReplicaDto {
            label,
            active,
            supervisor_id: _,
            emails,
            phones,
            location,
            staff: _,
        } = value;
        if !active {
            err_detail.push("not-active");
        }
        if label.is_empty() {
            err_detail.push("label-empty");
        }
        if let Some(ems) = &emails {
            if ems.is_empty() {
                err_detail.push("missing-email");
            }
        } else {
            err_detail.push("missing-email");
        }
        if let Some(phs) = &phones {
            if phs.is_empty() {
                err_detail.push("missing-phone");
            }
        } else {
            err_detail.push("missing-phone");
        }
        if location.is_none() {
            err_detail.push("missing-location-addr");
        }
        if err_detail.is_empty() {
            let mut emails = emails.unwrap();
            let mut phones = phones.unwrap();
            let location = location.unwrap();
            let type_ = ConnectAccountType::default();
            let capabilities = AccountCapabilityRequest::default();
            let tos_acceptance = AccountToSAccept::default();
            let country = location.country.clone();
            let email = emails.remove(0).addr;
            let business_profile = {
                let phone = {
                    let v = phones.remove(0);
                    v.line_number + "-" + v.country_code.as_str()
                };
                let email2 = emails.first().map_or(email.clone(), |v| v.addr.to_string());
                let args = ([label, email2, phone], location);
                AccountBusinessProfile::from(args)
            };
            let out = Self {
                type_,
                country,
                capabilities,
                email,
                tos_acceptance,
                business_profile,
                settings: AccountSettings::default(),
            };
            Ok(out)
        } else {
            let e = err_detail
                .into_iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            Err(AppProcessorErrorReason::InvalidStoreProfileDto(e))
        }
    } // end of fn try-from
} // end of impl CreateConnectAccount

impl ConnectAccount {
    pub(super) fn onboarding_complete(&self) -> bool {
        // in this payment service , Stripe account is applied only for
        // representing merchants / shop owners, I only consider whether
        // payout is enabled at here.
        let tx_active = matches!(
            self.capabilities.transfers,
            StripeAccountCapableState::active
        );
        self.details_submitted
            && self.payouts_enabled
            && self.tos_acceptance.date.is_some()
            && tx_active
    }
}

impl Default for AccountLinkType {
    fn default() -> Self {
        Self::account_onboarding
    }
}
impl<'a> From<(StoreOnboardStripeReqDto, &'a str)> for CreateAccountLink {
    fn from(value: (StoreOnboardStripeReqDto, &'a str)) -> Self {
        let (req, account) = value;
        let StoreOnboardStripeReqDto {
            refresh_url,
            return_url,
        } = req;
        let account = account.to_string();
        let type_ = AccountLinkType::default();
        Self {
            refresh_url,
            return_url,
            account,
            type_,
        }
    }
}

impl<'a, 'b> TryFrom<(&'a PayoutInnerModel, &'b Payout3partyStripeModel)> for CreateTransfer {
    type Error = AppProcessorErrorReason;

    fn try_from(
        value: (&'a PayoutInnerModel, &'b Payout3partyStripeModel),
    ) -> Result<Self, Self::Error> {
        let (pm, p3pt) = value;
        let amt_orig = pm.amount_base();
        let amt_represent = Payout3partyStripeModel::amount_represent(amt_orig, CurrencyDto::USD)
            .map_err(AppProcessorErrorReason::AmountOverflow)?;
        Ok(Self {
            amount: amt_represent,
            currency: CurrencyDto::USD,
            destination: p3pt.connect_account().to_string(),
            transfer_group: p3pt.transfer_group().to_string(),
        })
    } // end of fn from
} // end of impl CreateTransfer

impl Transfer {
    pub(super) fn amount_decimal(&self) -> Decimal {
        let scale = CurrencyDto::USD.amount_fraction_scale();
        Decimal::new(self.amount, scale)
    }
} // end of impl CreateTransfer

impl<'a, 'b> TryFrom<(&'a RefundReqRslvInnerModel, &'b Charge3partyStripeModel)> for CreateRefund {
    type Error = AppProcessorErrorReason;
    fn try_from(
        value: (&'a RefundReqRslvInnerModel, &'b Charge3partyStripeModel),
    ) -> Result<Self, Self::Error> {
        let (rslv_inner, chrg_3pty) = value;
        if chrg_3pty.payment_intent_id.is_empty() {
            let msg = "missing-payment-intent".to_string();
            return Err(AppProcessorErrorReason::ThirdParty(msg));
        }
        let currency_buyer = rslv_inner.currency()[0];
        let currency_label = currency_buyer.label.clone();
        let amt_orig = rslv_inner.total_amount_curr_round();
        let amt_final = Charge3partyStripeModel::amount_represent(amt_orig, currency_label)
            .map_err(AppProcessorErrorReason::AmountOverflow)?;
        Ok(Self {
            payment_intent: chrg_3pty.payment_intent_id.clone(),
            amount: amt_final.trunc_with_scale(0).to_string(),
            reason: RefundReason::requested_by_customer,
        })
    }
} // end of impl CreateRefund

impl RefundResult {
    pub(super) fn validate(&self, req: &CreateRefund) -> Result<(), AppProcessorErrorReason> {
        let result = if self.id.is_empty() {
            Err("missing-refund-id".to_string())
        } else if self.payment_intent != req.payment_intent {
            Err(format!(
                "corrupted-payment-intent:{}",
                self.payment_intent.as_str()
            ))
        } else if !matches!(self.status, RefundStatus::succeeded) {
            Err(format!("refund-status:{:?}", self.status))
        } else {
            Ok(())
        };
        result.map_err(AppProcessorErrorReason::ThirdParty)
    }
}
