use serde::de::{Error as DeserializeError, Expected, Unexpected};
use serde::{Deserialize, Serialize};

use crate::constant::ProductType;

#[derive(Deserialize, Serialize)]
pub struct PayAmountDto {
    pub unit: u32,
    pub total: u32,
}

#[derive(Deserialize, Serialize)]
pub struct GenericRangeErrorDto {
    pub max_: u16,
    pub min_: u16,
    pub given: u32,
}

struct ExpectProdTyp {
    numbers: Vec<u8>,
}
impl Expected for ExpectProdTyp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s: Vec<String> = self.numbers.iter().map(|n| n.to_string()).collect();
        let s = s.join(",");
        let msg = format!("accepted type number : {s}");
        formatter.write_str(msg.as_str())
    }
}

pub fn jsn_validate_product_type<'de, D>(raw: D) -> Result<ProductType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match u8::deserialize(raw) {
        Ok(d) => {
            let typ = ProductType::from(d);
            if let ProductType::Unknown(uv) = typ {
                let unexp = Unexpected::Unsigned(uv as u64);
                let exp = ExpectProdTyp {
                    numbers: vec![ProductType::Item.into(), ProductType::Package.into()],
                };
                let e = DeserializeError::invalid_value(unexp, &exp);
                Err(e)
            } else {
                Ok(typ)
            }
        }
        Err(e) => Err(e),
    }
}

pub fn jsn_serialize_product_type<S>(orig: &ProductType, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let v = orig.clone().into();
    ser.serialize_u8(v)
}


#[derive(Deserialize, Serialize)]
pub struct PhoneNumberDto {
    pub nation: u16,
    pub number: String,
}
#[derive(Deserialize, Serialize)]
pub struct ContactDto {
    pub first_name: String,
    pub last_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<PhoneNumberDto>,
}

#[derive(Deserialize, Serialize)]
pub enum CountryCode {
    TW,
    TH,
    IN,
    ID,
    US,
    Unknown,
}
impl From<CountryCode> for String {
    fn from(value: CountryCode) -> String {
        let out = match value {
            CountryCode::TW => "TW",
            CountryCode::TH => "TH",
            CountryCode::IN => "IN",
            CountryCode::ID => "ID",
            CountryCode::US => "US",
            CountryCode::Unknown => "Unknown",
        };
        out.to_string()
    }
} // implement `Into` trait, not replying on serde
impl From<String> for CountryCode {
    fn from(value: String) -> Self {
        match value.as_str() {
            "TW" => Self::TW,
            "TH" => Self::TH,
            "IN" => Self::IN,
            "ID" => Self::ID,
            "US" => Self::US,
            _others => Self::Unknown,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PhyAddrDto {
    pub country: CountryCode,
    pub region: String,
    pub city: String,
    pub distinct: String,
    pub street_name: Option<String>,
    pub detail: String,
}

#[derive(Deserialize, Serialize)]
pub struct BillingDto {
    pub contact: ContactDto,
    pub address: Option<PhyAddrDto>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(
        deserialize_with = "jsn_validate_product_type",
        serialize_with = "jsn_serialize_product_type"
    )]
    pub product_type: ProductType,
    pub reserved_until: String, // date-time formatted in RFC3339 spec
    pub quantity: u32,
    pub amount: PayAmountDto,
}