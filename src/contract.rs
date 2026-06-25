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
    #[doc(hidden)]
    OpenBridge {
        id: String,
        source_address: String,
        target_address: String,
        ack: bool,
    },
    #[doc(hidden)]
    CloseBridge {
        id: String,
        target_address: String,
        ack: bool,
    },
    #[doc(hidden)]
    ForwardText {
        id: String,
        topic: String,
        payload: String,
        local_only: bool,
        visited: Vec<String>,
        sequence: u64,
    },
    #[doc(hidden)]
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
    pub body: Body,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Body {
    Text(String),
    Binary(Vec<u8>),
}

impl Default for Body {
    fn default() -> Self {
        Body::Text(String::new())
    }
}

impl Message {
    /// The text payload, or `None` if this message carries binary data.
    pub fn text(&self) -> Option<&str> {
        match &self.body {
            Body::Text(text) => Some(text),
            Body::Binary(_) => None,
        }
    }

    /// The binary payload, or `None` if this message carries text.
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.body {
            Body::Binary(bytes) => Some(bytes),
            Body::Text(_) => None,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Route {
    #[default]
    Global,

    Local,
}
