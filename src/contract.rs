use enum2contract::EnumContract;
use serde::{Deserialize, Serialize};

#[derive(Debug, EnumContract, Serialize, Deserialize)]
pub enum BrokerContract {
    #[topic("hearsay/peers/connected")]
    PeerConnected { id: String },

    #[topic("hearsay/peers/request")]
    RequestPeers,

    #[topic("hearsay/peers/report")]
    ReportPeers { peers: Vec<String> },

    #[topic("hearsay/subscriptions/request")]
    RequestSubscriptions,

    #[topic("hearsay/subscriptions/report")]
    ReportSubscriptions {
        subscriptions: Vec<(String, Vec<String>)>,
    },

    #[topic("hearsay/bridges/created")]
    BridgeCreated {
        id: String,
        source_address: String,
        target_address: String,
    },

    #[topic("hearsay/bridges/request")]
    RequestBridges,

    #[topic("hearsay/bridges/report")]
    ReportBridges { bridges: Vec<String> },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PeerEvent {
    Hello {
        id: String,
    },
    Subscribe {
        id: String,
        topic: String,
    },
    Unsubscribe {
        id: String,
        topic: String,
    },
    PublishText {
        id: String,
        topic: String,
        payload: String,
        local_only: bool,
    },
    PublishBinary {
        id: String,
        topic: String,
        payload: Vec<u8>,
        local_only: bool,
    },
    OpenBridge {
        id: String,
        source_address: String,
        target_address: String,
        ack: bool,
    },
    CloseBridge {
        id: String,
        target_address: String,
        ack: bool,
    },
    ForwardText {
        id: String,
        topic: String,
        payload: String,
        local_only: bool,
        visited: Vec<String>,
        sequence: u64,
    },
    ForwardBinary {
        id: String,
        topic: String,
        payload: Vec<u8>,
        local_only: bool,
        visited: Vec<String>,
        sequence: u64,
    },
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct Message {
    pub topic: String,
    pub payload: String,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Route {
    #[default]
    Global,

    Local,
}
