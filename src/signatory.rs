use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mapping::TryIntoCdk;
use crate::trezor::handle_trezor_call;
use cdk_common::nuts::{BlindSignature, BlindedMessage, CurrencyUnit, Proof};
use cdk_common::{Error, Keys};
use cdk_signatory::signatory::{RotateKeyArguments, Signatory, SignatoryKeySet, SignatoryKeysets};
use trezor_client::{Trezor, TrezorMessage, TrezorResponse, protos};

#[derive(Clone)]
pub struct TrezorSignatory {
    pub trezor: Arc<Mutex<Trezor>>,
    pub cached_keysets: Option<SignatoryKeysets>,
}

impl TrezorSignatory {
    pub async fn new(trezor: Arc<Mutex<Trezor>>) -> Result<Self, Error> {
        Ok(Self {
            trezor,
            cached_keysets: None,
        })
    }

    pub async fn update_cached_keysets(&mut self) -> Result<(), Error> {
        self.cached_keysets = Some(self.keysets().await?);
        Ok(())
    }

    pub fn get_cached_keysets_proto(&self) -> Result<Vec<protos::KeySet>, Error> {
        if let Some(keysets) = &self.cached_keysets {
            return keysets
                .keysets
                .iter()
                .map(|ks| {
                    let mut ks2 = ks.clone();
                    //ks2.keys = Keys::new(BTreeMap::new());
                    ks2.try_into_cdk()
                })
                .collect::<Result<Vec<_>, Error>>();
        } else {
            return Err(Error::Custom("Keysets must be cached".to_string()));
        }
    }
}

#[async_trait::async_trait]
impl Signatory for TrezorSignatory {
    fn name(&self) -> String {
        format!("Trezor Signatory {}", env!("CARGO_PKG_VERSION"))
    }

    async fn blind_sign(
        &self,
        blinded_messages: Vec<BlindedMessage>,
    ) -> Result<Vec<BlindSignature>, Error> {
        let mut req = protos::CashuBlindSign::new();
        req.blinded_messages = blinded_messages
            .into_iter()
            .map(|bm| bm.try_into_cdk())
            .collect::<Result<Vec<_>, Error>>()?;
        req.set_operation(protos::Operation::OPERATION_UNSPECIFIED);
        req.keysets = self.get_cached_keysets_proto()?;

        let mut trezor = self.trezor.lock().await;
        let result = handle_trezor_call(
            trezor.call(req, Box::new(|_, m: protos::CashuBlindSignResponse| Ok(m))),
        )?;
        result.try_into_cdk()
    }

    async fn verify_proofs(&self, proofs: Vec<Proof>) -> Result<(), Error> {
        let mut req = protos::CashuVerifyProofs::new();
        let mut proofs_msg = protos::Proofs::new();
        proofs_msg.proof = proofs
            .into_iter()
            .map(|p| p.try_into_cdk())
            .collect::<Result<Vec<_>, Error>>()?;
        proofs_msg.set_operation(protos::Operation::OPERATION_UNSPECIFIED);
        proofs_msg.set_correlation_id("verify".to_string());
        req.proofs = ::protobuf::MessageField::some(proofs_msg);
        req.keysets = self.get_cached_keysets_proto()?;

        let mut trezor = self.trezor.lock().await;
        handle_trezor_call(trezor.call(req, Box::new(|_, m: protos::Success| Ok(m))))?;
        Ok(())
    }

    async fn keysets(&self) -> Result<SignatoryKeysets, Error> {
        let req = protos::CashuGetKeysets::new();

        // keysets will be the same for the lifetime of the device connection, so we can cache them
        if let Some(cached) = &self.cached_keysets {
            return Ok(cached.clone());
        }

        let mut trezor = self.trezor.lock().await;
        let result = handle_trezor_call(
            trezor.call(req, Box::new(|_, m: protos::CashuGetKeysetsResponse| Ok(m))),
        )?;

        let keysets = result
            .keysets
            .into_option()
            .ok_or(Error::Custom("missing keysets in response".to_string()))?;
        keysets.try_into_cdk()
    }

    async fn rotate_keyset(&self, _args: RotateKeyArguments) -> Result<SignatoryKeySet, Error> {
        Err(Error::Custom("Operation not supported".to_string()))
    }
}
