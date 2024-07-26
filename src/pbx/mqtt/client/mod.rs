use crate::prelude::*;
use paho_mqtt::{
    async_client::AsyncClient, ConnectOptionsBuilder, ConnectToken, CreateOptionsBuilder, DeliveryToken, Message, SubscribeToken
};
use pbni::{pbx::*, prelude::*};
use reactor::*;
use std::{mem::take, time::Duration};
use tokio::time;

mod config;
mod message;

use config::{MqttConfig, MqttConfigEx};
use message::MqttMessage;

struct Subscribe {
    topic_filter: String,
    qos: i32
}

struct MqttClient {
    state: HandlerState,
    client: Option<AsyncClient>,
    cfg: MqttConfigEx,
    has_connected: bool,
    has_closed: bool,
    conn_id: u64,
    offline_publish: Vec<Message>
}

#[nonvisualobject(name = "nx_mqttclient")]
impl MqttClient {
    #[constructor]
    fn new(session: Session, _object: Object) -> Self {
        MqttClient {
            state: HandlerState::new(session),
            client: None,
            cfg: Default::default(),
            has_connected: false,
            has_closed: false,
            conn_id: 0,
            offline_publish: Default::default()
        }
    }

    #[method(name = "IsOpen")]
    fn is_open(&mut self) -> bool {
        self.has_connected && self.client.as_ref().map(|client| client.is_connected()).unwrap_or_default()
    }

    #[method(name = "IsClosed")]
    fn is_closed(&mut self) -> bool { !self.is_open() }

    #[method(name = "Open", overload = 1)]
    fn open(&mut self, url: String, cfg: Option<&mut MqttConfig>) -> RetCode {
        if self.client.is_some() {
            return RetCode::E_BUSY;
        }
        let (create_cfg, conn_cfg, cfg) = match cfg {
            Some(cfg) => cfg.build(url),
            None => {
                let mut conn_builder = ConnectOptionsBuilder::default();
                conn_builder.server_uris(&url.split(";").collect::<Vec<&str>>());
                (CreateOptionsBuilder::default().finalize(), conn_builder.finalize(), Default::default())
            }
        };
        let client = AsyncClient::new(create_cfg)?;
        let invoker = self.invoker();
        client.set_connected_callback({
            let invoker = invoker.clone();
            move |_| {
                let invoker = invoker.clone();
                runtime::spawn(async move {
                    let _ = invoker
                        .invoke((), |this, ()| {
                            if this.client.is_none() {
                                return;
                            }
                            this.has_closed = false;
                            let is_reconnect = if !this.has_connected {
                                this.has_connected = true;
                                false
                            } else {
                                true
                            };
                            //TODO - 支持`session present`检测
                            this.on_open(is_reconnect, false);
                            //处理离线消息
                            let client = this.client.as_ref().unwrap(); //SAFETY
                            if !this.offline_publish.is_empty() {
                                let offline_publish = take(&mut this.offline_publish);
                                for msg in offline_publish {
                                    this.watch_publish(msg.topic().to_owned(), client.publish(msg));
                                }
                            }
                        })
                        .await;
                });
            }
        });
        client.set_disconnected_callback({
            let invoker = invoker.clone();
            move |_, _, reason| {
                let invoker = invoker.clone();
                runtime::spawn(async move {
                    let _ = invoker
                        .invoke((reason as pblong, reason.to_string()), |this, (code, info)| {
                            this.has_connected = false;
                            this.has_closed = true;
                            this.client = None;
                            this.on_close(code, info);
                        })
                        .await;
                });
            }
        });
        client.set_connection_lost_callback({
            let invoker = invoker.clone();
            move |_| {
                let invoker = invoker.clone();
                runtime::spawn(async move {
                    let _ = invoker
                        .invoke((), |this, ()| {
                            this.has_closed = true;
                            this.on_close(-1, "lost".to_owned());
                        })
                        .await;
                });
            }
        });
        client.set_message_callback({
            let invoker = invoker.clone();
            move |_, msg| {
                if let Some(msg) = msg {
                    let invoker = invoker.clone();
                    runtime::spawn(async move {
                        let _ = invoker
                            .invoke(msg, |this, msg| {
                                let obj =
                                    MqttMessage::new_object_modify(this.get_session(), |obj| obj.init(msg));
                                this.on_message(obj);
                            })
                            .await;
                    });
                }
            }
        });
        let token = client.connect(conn_cfg);
        self.client = Some(client);
        self.cfg = cfg;
        self.conn_id += 1;
        self.watch_connect(token);

        RetCode::OK
    }

    #[method(name = "Close")]
    fn close(&mut self) -> RetCode {
        self.offline_publish.clear();
        let has_connected = self.has_connected;
        let has_closed = self.has_closed;
        self.has_connected = false;
        self.has_closed = false;
        if let Some(client) = self.client.take() {
            runtime::spawn(async move {
                let _ = time::timeout(Duration::from_secs(3), client.disconnect(None)).await;
            });
            if has_connected && !has_closed {
                self.on_close(0, "close".to_owned());
            }
        }
        RetCode::OK
    }

    #[method(name = "Publish")]
    fn publish(&mut self, msg: &mut MqttMessage) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let msg = match msg.take() {
                Some(msg) => msg,
                None => return RetCode::E_INVALID_OBJECT
            };
            if (self.has_connected || !self.cfg.offline_queue) && client.is_connected() {
                self.watch_publish(msg.topic().to_owned(), client.publish(msg));
            } else if self.cfg.offline_queue {
                self.offline_publish.push(msg);
            } else {
                return RetCode::E_IO_ERROR;
            }
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Subscribe", overload = 1)]
    fn subscribe(&mut self, topic_filter: String, qos: Option<pblong>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let qos = qos.unwrap_or_default();
            self.watch_subscribe(topic_filter.clone(), client.subscribe(topic_filter, qos));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Subscribe", overload = 1)]
    fn subscribe_many(&mut self, topic_filters: Vec<String>, qos: Option<Vec<pblong>>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            let qos = qos.unwrap_or_else(|| {
                let mut qos = Vec::with_capacity(topic_filters.len());
                qos.resize(topic_filters.len(), 0);
                qos
            });
            assert_eq!(topic_filters.len(), qos.len());
            self.watch_subscribe(topic_filters.join(";"), client.subscribe_many(&topic_filters, &qos));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Unsubscribe")]
    fn unsubscribe(&mut self, topic_filter: String) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            self.watch_unsubscribe(topic_filter.clone(), client.unsubscribe(topic_filter));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    #[method(name = "Unsubscribe")]
    fn unsubscribe_many(&mut self, topic_filters: Vec<String>) -> RetCode {
        if let Some(client) = self.client.as_ref() {
            self.watch_unsubscribe(topic_filters.join(";"), client.unsubscribe_many(&topic_filters));
            RetCode::OK
        } else {
            RetCode::E_INVALID_HANDLE
        }
    }

    fn watch_connect(&self, token: ConnectToken) {
        let conn_id = self.conn_id;
        self.spawn(async move { token.await }, move |this, rv| {
            if this.client.is_some() && conn_id == this.conn_id {
                if let Err(e) = rv {
                    this.client = None;
                    this.on_error(error_code::ERROR_CONNECT, format!("connect error: {e}"));
                }
            }
        });
    }

    fn watch_publish(&self, topic: String, token: DeliveryToken) {
        let conn_id = self.conn_id;
        self.spawn(async move { token.await }, move |this, rv| {
            if this.client.is_some() && conn_id == this.conn_id {
                if let Err(e) = rv {
                    this.on_error(error_code::ERROR_PUBLISH, format!("publish error: {topic}, {e}"));
                }
            }
        });
    }

    fn watch_subscribe(&self, topic_filters: String, token: SubscribeToken) {
        let conn_id = self.conn_id;
        self.spawn(async move { token.await }, move |this, rv| {
            if this.client.is_some() && conn_id == this.conn_id {
                if let Err(e) = rv {
                    this.on_error(
                        error_code::ERROR_SUBSCRIBE,
                        format!("subscribe error: {topic_filters}, {e}")
                    );
                }
            }
        });
    }

    fn watch_unsubscribe(&self, topic_filters: String, token: SubscribeToken) {
        let conn_id = self.conn_id;
        self.spawn(async move { token.await }, move |this, rv| {
            if this.client.is_some() && conn_id == this.conn_id {
                if let Err(e) = rv {
                    this.on_error(
                        error_code::ERROR_UNSUBSCRIBE,
                        format!("unsubscribe error: {topic_filters}, {e}")
                    );
                }
            }
        });
    }

    #[event(name = "OnOpen")]
    fn on_open(&mut self, reconnect: bool, session_present: bool) {}

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

mod error_code {
    use super::*;

    pub const ERROR_CONNECT: pblong = -1;
    pub const ERROR_PUBLISH: pblong = -2;
    pub const ERROR_SUBSCRIBE: pblong = -3;
    pub const ERROR_UNSUBSCRIBE: pblong = -4;
}
