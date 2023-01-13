use crate::{base::pfw, prelude::*};
use paho_mqtt::{
    async_client::AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, DeliveryToken, Message, SubscribeToken
};
use pbni::{pbx::*, prelude::*};
use reactor::*;

mod config;
mod message;

use config::MqttConfig;

use self::message::MqttMessage;

struct MqttClient {
    state: HandlerState,
    client: Option<AsyncClient>,
    has_connected: bool
}

#[nonvisualobject(name = "nx_mqttclient")]
impl MqttClient {
    #[constructor]
    fn new(session: Session, _object: Object) -> Self {
        MqttClient {
            state: HandlerState::new(session),
            client: None,
            has_connected: false
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
                let _ = invoker.invoke_blocking((), |this, _| {
                    let is_reconnect = if !this.has_connected {
                        this.has_connected = true;
                        false
                    } else {
                        true
                    };
                    this.on_open(is_reconnect);
                });
            }
        });
        client.set_disconnected_callback({
            let invoker = invoker.clone();
            move |_, _, reason| {
                let _ =
                    invoker.invoke_blocking((reason as pblong, reason.to_string()), |this, (code, info)| {
                        this.on_close(code, info);
                    });
            }
        });
        client.set_connection_lost_callback({
            let invoker = invoker.clone();
            move |_| {
                let _ = invoker.invoke_blocking((-1, "lost".to_owned()), |this, (code, info)| {
                    this.on_close(code, info);
                });
            }
        });
        client.set_message_callback({
            let invoker = invoker.clone();
            move |_, msg| {
                if let Some(msg) = msg {
                    let _ = invoker.invoke_blocking(msg, |this, msg| {
                        let obj =
                            MqttMessage::new_object_modify(this.get_session(), |obj| obj.init(msg)).unwrap();
                        this.on_message(obj);
                    });
                }
            }
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
        self.has_connected = false;
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

    #[method(name = "Subscribe", overload = 1)]
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

    #[method(name = "Subscribe", overload = 1)]
    fn subscribe_many(&mut self, topic_filters: Vec<String>, qos: Option<Vec<pblong>>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            client.subscribe_many(&topic_filters, &qos.unwrap_or_default());
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Unsubscribe")]
    fn unsubscribe(&mut self, topic_filter: String) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            client.unsubscribe(topic_filter);
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Unsubscribe")]
    fn unsubscribe_many(&mut self, topic_filters: Vec<String>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            client.unsubscribe_many(&topic_filters);
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

    #[event(name = "OnOpen")]
    fn on_open(&mut self, reconnect: bool) {}

    #[event(name = "OnClose")]
    fn on_close(&mut self, code: pblong, info: String) {}

    #[event(name = "OnError")]
    fn on_error(&mut self, code: pblong, info: String) {}

    #[event(name = "OnMessage")]
    fn on_message(&mut self, msg: Object) {}
}

impl Handler for MqttClient {
    fn state(&self) -> &HandlerState { &self.state }
    fn alive_state(&self) -> AliveState { self.get_alive_state() }
}
