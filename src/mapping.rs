use anyhow::Result;
use cdk_common::nuts::{BlindSignature, BlindedMessage, CurrencyUnit, Id, MintKeySet, Proof};
use cdk_common::{Amount, BlindSignatureDleq, Error, PublicKey, SecretKey};
use cdk_signatory::signatory::{RotateKeyArguments, Signatory, SignatoryKeySet, SignatoryKeysets};
use trezor_client::{Trezor, TrezorResponse, protos};

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

impl TryIntoCdk<Vec<BlindSignature>>
    for TrezorResponse<'_, protos::CashuBlindSignResponse, protos::CashuBlindSignResponse>
{
    fn try_into_cdk(self) -> Result<Vec<BlindSignature>, Error> {
        match self {
            TrezorResponse::Ok(res) => res.sigs.into_iter().map(|sig| sig.try_into_cdk()).collect(),
            _ => Err(Error::Custom("Trezor operation failed".to_string())),
        }
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
