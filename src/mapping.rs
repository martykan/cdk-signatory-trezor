use anyhow::Result;
use cdk_common::nuts::{BlindSignature, BlindedMessage, CurrencyUnit, Id, Keys, Proof};
use cdk_common::{Amount, BlindSignatureDleq, Error, PublicKey, SecretKey};
use cdk_signatory::signatory::{SignatoryKeySet, SignatoryKeysets};
use protobuf::MessageField;
use trezor_client::{TrezorResponse, protos};

/// Trait for converting Trezor protobuf types to CDK types.
///
/// We use a custom trait instead of `TryFrom` because both the source and target
/// types are from external crates (orphan rules).
pub trait TryIntoCdk<T> {
    fn try_into_cdk(self) -> Result<T, Error>;
}

/// Helper to extract a required field from Option with a descriptive error
#[inline]
fn required<T>(opt: Option<T>, field: &str) -> Result<T, Error> {
    opt.ok_or_else(|| Error::Custom(format!("missing required field: {}", field)))
}

impl TryIntoCdk<BlindSignatureDleq> for protos::BlindSignatureDLEQ {
    fn try_into_cdk(self) -> Result<BlindSignatureDleq, Error> {
        Ok(BlindSignatureDleq {
            e: SecretKey::from_slice(&required(self.e, "e")?)?,
            s: SecretKey::from_slice(&required(self.s, "s")?)?,
        })
    }
}

impl TryIntoCdk<BlindSignature> for protos::BlindSignature {
    fn try_into_cdk(self) -> Result<BlindSignature, Error> {
        Ok(BlindSignature {
            amount: required(self.amount, "amount")?.into(),
            keyset_id: Id::from_bytes(&required(self.keyset_id, "keyset_id")?)?,
            c: PublicKey::from_slice(&required(self.blinded_secret, "blinded_secret")?)?,
            dleq: self
                .dleq
                .into_option()
                .map(|d| d.try_into_cdk())
                .transpose()?,
        })
    }
}

impl TryIntoCdk<Vec<BlindSignature>> for protos::CashuBlindSignResponse {
    fn try_into_cdk(self) -> Result<Vec<BlindSignature>, Error> {
        self.sigs
            .into_iter()
            .map(|sig| sig.try_into_cdk())
            .collect()
    }
}

impl TryIntoCdk<protos::BlindedMessage> for BlindedMessage {
    fn try_into_cdk(self) -> Result<protos::BlindedMessage, Error> {
        Ok(protos::BlindedMessage {
            amount: Some(self.amount.into()),
            keyset_id: Some(self.keyset_id.to_bytes()),
            blinded_secret: Some(self.blinded_secret.to_bytes().to_vec()),
            special_fields: Default::default(),
        })
    }
}

// Convert to/from Trezor protos to CDK types for keysets
impl TryIntoCdk<protos::KeySet> for SignatoryKeySet {
    fn try_into_cdk(self) -> Result<protos::KeySet, Error> {
        Ok(protos::KeySet {
            id: Some(self.id.to_bytes()),
            unit: MessageField::some(protos::CurrencyUnit {
                currency_unit: Some(match self.unit {
                    CurrencyUnit::Sat => protos::currency_unit::Currency_unit::Unit(
                        protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_SAT.into(),
                    ),
                    CurrencyUnit::Msat => protos::currency_unit::Currency_unit::Unit(
                        protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_MSAT.into(),
                    ),
                    CurrencyUnit::Usd => protos::currency_unit::Currency_unit::Unit(
                        protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_USD.into(),
                    ),
                    CurrencyUnit::Eur => protos::currency_unit::Currency_unit::Unit(
                        protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_EUR.into(),
                    ),
                    CurrencyUnit::Auth => protos::currency_unit::Currency_unit::Unit(
                        protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_AUTH.into(),
                    ),
                    CurrencyUnit::Custom(s) => protos::currency_unit::Currency_unit::CustomUnit(s),
                    _ => {
                        return Err(Error::UnsupportedUnit);
                    }
                }),
                special_fields: Default::default(),
            }),
            active: Some(self.active),
            input_fee_ppk: Some(self.input_fee_ppk),
            keys: MessageField::some(protos::Keys {
                keys: self
                    .keys
                    .iter()
                    .map(|(amount, pk)| (amount.to_u64(), pk.to_bytes().to_vec()))
                    .collect(),
                special_fields: Default::default(),
            }),
            final_expiry: self.final_expiry,
            version: Some(1),
            special_fields: Default::default(),
        })
    }
}

impl TryIntoCdk<SignatoryKeysets> for protos::SignatoryKeysets {
    fn try_into_cdk(self) -> Result<SignatoryKeysets, Error> {
        Ok(SignatoryKeysets {
            pubkey: PublicKey::from_slice(&required(self.pubkey, "pubkey")?)?,
            keysets: self
                .keysets
                .into_iter()
                .map(|ks| ks.try_into_cdk())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryIntoCdk<SignatoryKeySet> for protos::KeySet {
    fn try_into_cdk(self) -> Result<SignatoryKeySet, Error> {
        let unit = required(self.unit.into_option(), "unit")?;
        let currency_unit = match unit.currency_unit {
            Some(protos::currency_unit::Currency_unit::Unit(u)) => {
                match u.enum_value_or_default() {
                    protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_SAT => CurrencyUnit::Sat,
                    protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_MSAT => CurrencyUnit::Msat,
                    protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_USD => CurrencyUnit::Usd,
                    protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_EUR => CurrencyUnit::Eur,
                    protos::CurrencyUnitType::CURRENCY_UNIT_TYPE_AUTH => CurrencyUnit::Auth,
                    _ => return Err(Error::UnsupportedUnit),
                }
            }
            Some(protos::currency_unit::Currency_unit::CustomUnit(s)) => CurrencyUnit::Custom(s),
            err => {
                return Err(Error::Custom(
                    format!("missing or invalid currency unit: {:?}", err).to_string(),
                ));
            }
        };

        let keys_proto = required(self.keys.into_option(), "keys")?;
        let keys_map: std::collections::BTreeMap<Amount, PublicKey> = keys_proto
            .keys
            .into_iter()
            .map(|(amount, pubkey_bytes)| {
                Ok((Amount::from(amount), PublicKey::from_slice(&pubkey_bytes)?))
            })
            .collect::<Result<_, Error>>()?;

        let amounts: Vec<u64> = keys_map.keys().map(|a| (*a).into()).collect();

        Ok(SignatoryKeySet {
            id: Id::from_bytes(&required(self.id, "id")?)?,
            unit: currency_unit,
            active: required(self.active, "active")?,
            keys: Keys::new(keys_map),
            amounts,
            input_fee_ppk: required(self.input_fee_ppk, "input_fee_ppk")?,
            final_expiry: self.final_expiry,
        })
    }
}

// Convert from CDK types to Trezor protos for writing
impl TryIntoCdk<protos::Proof> for Proof {
    fn try_into_cdk(self) -> Result<protos::Proof, Error> {
        Ok(protos::Proof {
            amount: Some(self.amount.into()),
            keyset_id: Some(self.keyset_id.to_bytes()),
            secret: Some(self.secret.as_bytes().to_vec()),
            c: Some(self.c.to_bytes().to_vec()),
            special_fields: Default::default(),
        })
    }
}
