use crate::{base::pfw, prelude::*};
use paho_mqtt::{
    async_client::AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, DeliveryToken, Message, SubscribeToken
};
use pbni::{pbx::*, prelude::*};
use reactor::*;

mod config;

use config::MqttConfig;

struct MqttClient {
    state: HandlerState,
    client: Option<AsyncClient>
}

#[nonvisualobject(name = "nx_mqttclient")]
impl MqttClient {
    #[constructor]
    fn new(session: Session, _object: Object) -> Self {
        MqttClient {
            state: HandlerState::new(session),
            client: None
        }
    }

    #[method(name = "IsOpen")]
    fn is_open(&mut self) -> bool {
        self.client.as_ref().map(|client| client.is_connected()).unwrap_or_default()
    }

    #[method(name = "IsClosed")]
    fn is_closed(&mut self) -> bool { !self.is_open() }

    #[method(name = "IsPending")]
    fn is_pending(&mut self) -> bool { false }

    #[method(name = "IsReconnecting")]
    fn is_reconnecting(&mut self) -> bool { false }

    #[method(name = "GetState")]
    fn get_state(&mut self) -> pblong { 0 }

    #[method(name = "Open", overload = 1)]
    fn open(&mut self, url: String, cfg: Option<&mut MqttConfig>) -> RetCode {
        if self.client.is_some() {
            return RetCode::E_BUSY;
        }
        let (create_cfg, conn_cfg) = match cfg {
            Some(cfg) => cfg.build(url),
            None => {
                let mut conn_builder = ConnectOptionsBuilder::default();
                conn_builder.server_uris(&url.split(";").collect::<Vec<&str>>());
                (CreateOptionsBuilder::default().finalize(), conn_builder.finalize())
            }
        };
        let client = AsyncClient::new(create_cfg)?;
        let invoker = self.invoker();
        client.set_connected_callback({
            let invoker = invoker.clone();
            move |_| {
                let _ = invoker.blocking_invoke((false, false), |this, (reconnect, session_present)| {
                    this.on_open(reconnect, session_present);
                });
            }
        });
        client.set_disconnected_callback(move |_, props, reason| {
            let _ = invoker.blocking_invoke((0, String::new()), |this, (code, info)| {
                this.on_close(code, info);
            });
        });
        client.connect(conn_cfg);
        self.client = Some(client);

        RetCode::OK
    }

    #[method(name = "Close")]
    fn close(&mut self) -> RetCode {
        if let Some(client) = self.client.take() {
            client.disconnect(None);
        }
        RetCode::OK
    }

    #[method(name = "Publish", overload = 2)]
    fn publish(&mut self, topic: String, qos: Option<pblong>, retain: Option<bool>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let msg = if retain.unwrap_or_default() {
                Message::new(topic.clone(), Vec::new(), qos.unwrap_or_default())
            } else {
                Message::new_retained(topic.clone(), Vec::new(), qos.unwrap_or_default())
            };
            self.watch_publish(topic, client.publish(msg));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Publish", overload = 2)]
    fn publish_string(
        &mut self,
        topic: String,
        data: String,
        qos: Option<pblong>,
        retain: Option<bool>
    ) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let msg = if retain.unwrap_or_default() {
                Message::new(topic.clone(), data, qos.unwrap_or_default())
            } else {
                Message::new_retained(topic.clone(), data, qos.unwrap_or_default())
            };
            self.watch_publish(topic, client.publish(msg));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Publish", overload = 2)]
    fn publish_binary(
        &mut self,
        topic: String,
        data: &[u8],
        qos: Option<pblong>,
        retain: Option<bool>
    ) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let msg = if retain.unwrap_or_default() {
                Message::new(topic.clone(), data, qos.unwrap_or_default())
            } else {
                Message::new_retained(topic.clone(), data, qos.unwrap_or_default())
            };
            self.watch_publish(topic, client.publish(msg));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Publish", overload = 2)]
    fn publish_json_or_xml(
        &mut self,
        topic: String,
        obj: Object,
        qos: Option<pblong>,
        retain: Option<bool>
    ) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let data = match obj.get_class_name().as_str() {
                "n_json" => pfw::json_serialize(&obj),
                "n_xml" => pfw::xml_serialize(&obj),
                cls @ _ => panic!("unexpect class {cls}")
            };
            let msg = if retain.unwrap_or_default() {
                Message::new(topic.clone(), data, qos.unwrap_or_default())
            } else {
                Message::new_retained(topic.clone(), data, qos.unwrap_or_default())
            };
            self.watch_publish(topic, client.publish(msg));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Subscribe", overload = 2)]
    fn subscribe(&mut self, topic_filter: String, qos: Option<pblong>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            self.watch_subscribe(
                topic_filter.clone(),
                client.subscribe(topic_filter, qos.unwrap_or_default())
            );
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Subscribe", overload = 2)]
    fn subscribe_many(&mut self, topic_filters: Vec<String>, qos: Option<Vec<pblong>>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            client.subscribe_many(&topic_filters, &qos.unwrap_or_default());
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    fn watch_publish(&self, topic: String, token: DeliveryToken) {
        self.spawn(async move { token.await }, |this, rv| {});
    }

    fn watch_subscribe(&self, topic_filter: String, token: SubscribeToken) {
        self.spawn(async move { token.await }, |this, rv| {});
    }

    fn on_open(&mut self, reconnect: bool, session_present: bool) {
        let mut obj = self.get_object();
        let invoker = obj.begin_invoke_event("OnOpen").unwrap();
        invoker.arg(0).set_bool(reconnect);
        invoker.arg(1).set_bool(session_present);
        invoker.trigger().unwrap();
    }

    fn on_close(&mut self, code: pblong, info: String) {
        let mut obj = self.get_object();
        let invoker = obj.begin_invoke_event("OnClose").unwrap();
        invoker.arg(0).set_long(code);
        invoker.arg(1).set_str(info);
        invoker.trigger().unwrap();
    }
}

impl Handler for MqttClient {
    fn state(&self) -> &HandlerState { &self.state }
    fn alive_state(&self) -> AliveState { self.get_alive_state() }
}
