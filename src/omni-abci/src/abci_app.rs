use minicose::CoseSign1;
use omni::message::ResponseMessage;
use omni::server::module::abci_backend::{AbciBlock, AbciCommitInfo, AbciInfo};
use omni::types::identity::cose::CoseKeyIdentity;
use omni::{Identity, OmniError};
use omni_client::OmniClient;
use reqwest::{IntoUrl, Url};
use std::time::SystemTime;
use tendermint_abci::Application;
use tendermint_proto::abci::*;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct AbciApp {
    app_name: String,
    omni_client: OmniClient,
    omni_url: Url,
}

impl AbciApp {
    /// Constructor.
    pub fn create<U>(omni_url: U, server_id: Identity) -> Result<Self, String>
    where
        U: IntoUrl,
    {
        let omni_url = omni_url.into_url().map_err(|e| e.to_string())?;

        // TODO: Get the server ID from the omni server.
        // let server_id = if server_id.is_anonymous() {
        //     server_id
        // } else {
        //     server_id
        // };

        let omni_client =
            OmniClient::new(omni_url.clone(), server_id, CoseKeyIdentity::anonymous())
                .map_err(|e| e)?;
        let status = omni_client.status().map_err(|x| x.to_string())?;
        let app_name = status.name;

        Ok(Self {
            app_name,
            omni_url,
            omni_client,
        })
    }
}

impl Application for AbciApp {
    fn info(&self, request: RequestInfo) -> ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            request.version, request.block_version, request.p2p_version
        );

        let AbciInfo { height, hash } =
            match self.omni_client.call_("abci.info", ()).and_then(|payload| {
                minicbor::decode(&payload)
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))
            }) {
                Ok(x) => x,
                Err(err) => {
                    return ResponseInfo {
                        data: format!("An error occurred during call to abci.info:\n{}", err),
                        ..Default::default()
                    }
                }
            };

        ResponseInfo {
            data: format!("omni-abci-bridge({})", self.app_name),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_version: 1,
            last_block_height: height as i64,
            last_block_app_hash: hash.to_vec().into(),
        }
    }
    fn init_chain(&self, _request: RequestInitChain) -> ResponseInitChain {
        Default::default()
    }
    fn query(&self, request: RequestQuery) -> ResponseQuery {
        let cose = match CoseSign1::from_bytes(&request.data) {
            Ok(x) => x,
            Err(err) => {
                return ResponseQuery {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        let value = match OmniClient::send_envelope(self.omni_url.clone(), cose) {
            Ok(cose_sign) => cose_sign,

            Err(err) => {
                return ResponseQuery {
                    code: 3,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        match value.to_bytes() {
            Ok(value) => ResponseQuery {
                code: 0,
                value: value.into(),
                ..Default::default()
            },
            Err(err) => ResponseQuery {
                code: 1,
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn begin_block(&self, request: RequestBeginBlock) -> ResponseBeginBlock {
        let time = request
            .header
            .map(|x| x.time.map(|x| x.seconds as u64))
            .flatten();

        let block = AbciBlock { time };
        let _ = self.omni_client.call_("abci.beginBlock", block);
        ResponseBeginBlock { events: vec![] }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let cose = match CoseSign1::from_bytes(&request.tx) {
            Ok(x) => x,
            Err(err) => {
                return ResponseDeliverTx {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        match OmniClient::send_envelope(self.omni_url.clone(), cose) {
            Ok(cose_sign) => {
                let payload = cose_sign.payload.unwrap_or_default();
                let mut response = ResponseMessage::from_bytes(&payload).unwrap_or_default();

                // Consensus will sign the result, so the `from` field is unnecessary.
                response.from = Identity::anonymous();
                // The version is ignored and removed.
                response.version = None;
                // The timestamp MIGHT differ between two nodes so we just force it to be 0.
                response.timestamp = Some(SystemTime::UNIX_EPOCH);

                if let Ok(data) = response.to_bytes() {
                    ResponseDeliverTx {
                        code: 0,
                        data: data.into(),
                        ..Default::default()
                    }
                } else {
                    ResponseDeliverTx {
                        code: 3,
                        ..Default::default()
                    }
                }
            }
            Err(err) => ResponseDeliverTx {
                code: 1,
                data: vec![].into(),
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn end_block(&self, _request: RequestEndBlock) -> ResponseEndBlock {
        let _ = self.omni_client.call_("abci.endBlock", block);
        Default::default()
    }

    fn flush(&self) -> ResponseFlush {
        Default::default()
    }

    fn commit(&self) -> ResponseCommit {
        self.omni_client.call_("abci.commit", ()).map_or_else(
            |err| ResponseCommit {
                data: err.to_string().into_bytes().into(),
                retain_height: 0,
            },
            |msg| {
                let info: AbciCommitInfo = minicbor::decode(&msg).unwrap();
                ResponseCommit {
                    data: info.hash.to_vec().into(),
                    retain_height: info.retain_height as i64,
                }
            },
        )
    }
}
