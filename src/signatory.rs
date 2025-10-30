use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mapping::TryIntoCdk;
use cdk_common::nuts::{BlindSignature, BlindedMessage, CurrencyUnit, Id, MintKeySet, Proof};
use cdk_common::{Amount, Error};
use cdk_signatory::signatory::{RotateKeyArguments, Signatory, SignatoryKeySet, SignatoryKeysets};
use trezor_client::{Trezor, TrezorResponse, protos};

pub struct TrezorSignatory {
    pub trezor: Arc<Mutex<Trezor>>,
}

impl TrezorSignatory {
    pub async fn new(trezor: Arc<Mutex<Trezor>>) -> Result<Self, Error> {
        Ok(Self { trezor })
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

        let mut trezor = self.trezor.lock().await;
        let result = trezor.call(req, Box::new(|_, m: protos::CashuBlindSignResponse| Ok(m)));
        result.map_err(|_| Error::Internal)?.try_into_cdk()
    }

    async fn verify_proofs(&self, proofs: Vec<Proof>) -> Result<(), Error> {}

    async fn keysets(&self) -> Result<SignatoryKeysets, Error> {}

    async fn rotate_keyset(&self, args: RotateKeyArguments) -> Result<SignatoryKeySet, Error> {}
}
