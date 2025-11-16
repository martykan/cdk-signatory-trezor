use cdk_common::Error;
use trezor_client::{TrezorMessage, TrezorResponse};

/// Unwrap Trezor call responses and handle interaction requests
pub fn handle_trezor_call<T, R: TrezorMessage>(
    resp: Result<TrezorResponse<T, R>, trezor_client::Error>,
) -> Result<T, Error> {
    match resp {
        Err(err) => Err(Error::Custom(format!("Trezor call error: {:?}", err))),
        Ok(TrezorResponse::Ok(res)) => Ok(res),
        Ok(TrezorResponse::Failure(err)) => {
            Err(Error::Custom(format!("Trezor failure response: {:?}", err)))
        }
        Ok(TrezorResponse::ButtonRequest(req)) => handle_trezor_call(req.ack()),
        Ok(TrezorResponse::PinMatrixRequest(_)) => Err(Error::Custom(
            "Pin matrix request not supported".to_string(),
        )),
        Ok(TrezorResponse::PassphraseRequest(req)) => {
            // empty passphrase
            let pass = String::new();
            handle_trezor_call(req.ack_passphrase(pass.to_owned()))
        }
    }
}
