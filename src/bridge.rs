use crate::{
    Client, ClientSettings, Result, assign_client_id, client_id, connect, create_client,
    is_connected,
};

pub(crate) struct Bridge {
    pub(crate) client: Client,
    pub(crate) id: String,
    pub(crate) target_address: String,
}

pub(crate) async fn create_bridge(override_id: Option<String>, target_address: &str) -> Bridge {
    let settings = ClientSettings {
        autoreconnect: false,
        max_connection_attempts: None,
        ..Default::default()
    };
    let mut client = create_client("bridge", settings);
    if let Some(id) = override_id {
        assign_client_id(&mut client, &id).await;
    }
    let id = client_id(&client).await;
    Bridge {
        client,
        id,
        target_address: target_address.to_string(),
    }
}

pub(crate) async fn connect_bridge(bridge: &mut Bridge) -> Result<()> {
    connect(&mut bridge.client, &bridge.target_address).await
}

pub(crate) async fn bridge_is_connected(bridge: &Bridge) -> bool {
    is_connected(&bridge.client).await
}
