use super::*;
use crate::base::pfw;
use paho_mqtt::{
    ClientPersistence, ConnectOptions, ConnectOptionsBuilder, CreateOptions, CreateOptionsBuilder, Message, PersistenceType
};
use std::{collections::HashMap, mem::replace, time::Duration};

pub struct MqttConfig {
    create_builder: Option<CreateOptionsBuilder>,
    conn_builder: ConnectOptionsBuilder
}

impl Default for MqttConfig {
    fn default() -> Self {
        MqttConfig {
            create_builder: Some(CreateOptionsBuilder::default()),
            conn_builder: ConnectOptionsBuilder::default()
        }
    }
}

#[nonvisualobject(name = "nx_mqttconfig")]
impl MqttConfig {
    /// 创建`paho_mqtt::CreateOptions`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn build(&mut self, url: String) -> (CreateOptions, ConnectOptions) {
        let create_builder = self.create_builder.replace(CreateOptionsBuilder::default()).unwrap();
        let mut conn_builder = replace(&mut self.conn_builder, ConnectOptionsBuilder::default());
        conn_builder.server_uris(&url.split(";").collect::<Vec<&str>>());
        (create_builder.finalize(), conn_builder.finalize())
    }

    #[method(name = "SetVersion")]
    fn version(&mut self, ver: pblong) -> &mut Self {
        let create_builder = self.create_builder.take().unwrap();
        self.create_builder.replace(create_builder.mqtt_version(ver as u32));
        self
    }

    #[method(name = "SetClientId")]
    fn client_id(&mut self, id: String) -> &mut Self {
        let create_builder = self.create_builder.take().unwrap();
        self.create_builder.replace(create_builder.client_id(id));
        self
    }

    #[method(name = "SetCredential")]
    fn credential(&mut self, user: String, psw: String) -> &mut Self {
        self.conn_builder.user_name(user).password(psw);
        self
    }

    #[method(name = "SetCleanSession")]
    fn clean_session(&mut self, clean: bool) -> &mut Self {
        self.conn_builder.clean_session(clean);
        self
    }

    #[method(name = "SetPersistence")]
    fn persistence_enabled(&mut self, enabled: bool) -> &mut Self {
        let create_builder = self.create_builder.take().unwrap();
        if enabled {
            self.create_builder.replace(create_builder.user_persistence(RuntimeStore::default()));
        } else {
            self.create_builder.replace(create_builder.persistence(PersistenceType::None));
        }
        self
    }

    #[method(name = "SetPersistence")]
    fn persistence_file(&mut self, file_path: String) -> &mut Self {
        let create_builder = self.create_builder.take().unwrap();
        self.create_builder.replace(create_builder.persistence(file_path));
        self
    }

    #[method(name = "SetOfflineQueue")]
    fn offline_queue(&mut self, enabled: bool) -> &mut Self {
        let create_builder = self.create_builder.take().unwrap();
        self.create_builder.replace(create_builder.send_while_disconnected(enabled));
        self
    }

    #[method(name = "SetAutoReconnect")]
    fn automatic_reconnect(&mut self, enabled: bool) -> &mut Self {
        if enabled {
            self.conn_builder.automatic_reconnect(Duration::from_secs(1), Duration::from_secs(30));
        }
        self
    }

    #[method(name = "SetTimeout")]
    fn timeout(&mut self, secs: pbdouble) -> &mut Self {
        self.conn_builder.connect_timeout(Duration::from_secs_f64(secs));
        self
    }

    #[method(name = "WillMessage", overload = 2)]
    fn will_message(&mut self, topic: String, qos: Option<pblong>, retain: Option<bool>) -> &mut Self {
        let msg = if retain.unwrap_or_default() {
            Message::new_retained(topic.clone(), Vec::new(), qos.unwrap_or_default())
        } else {
            Message::new(topic.clone(), Vec::new(), qos.unwrap_or_default())
        };
        self.conn_builder.will_message(msg);
        self
    }

    #[method(name = "WillMessage", overload = 2)]
    fn will_message_string(
        &mut self,
        topic: String,
        data: String,
        qos: Option<pblong>,
        retain: Option<bool>
    ) -> &mut Self {
        let msg = if retain.unwrap_or_default() {
            Message::new_retained(topic.clone(), data, qos.unwrap_or_default())
        } else {
            Message::new(topic.clone(), data, qos.unwrap_or_default())
        };
        self.conn_builder.will_message(msg);
        self
    }

    #[method(name = "WillMessage", overload = 2)]
    fn will_message_binary(
        &mut self,
        topic: String,
        data: &[u8],
        qos: Option<pblong>,
        retain: Option<bool>
    ) -> &mut Self {
        let msg = if retain.unwrap_or_default() {
            Message::new_retained(topic.clone(), data, qos.unwrap_or_default())
        } else {
            Message::new(topic.clone(), data, qos.unwrap_or_default())
        };
        self.conn_builder.will_message(msg);
        self
    }

    #[method(name = "WillMessage", overload = 2)]
    fn will_message_json_or_xml(
        &mut self,
        topic: String,
        obj: Object,
        qos: Option<pblong>,
        retain: Option<bool>
    ) -> &mut Self {
        let data = match obj.get_class_name().as_str() {
            "n_json" => pfw::json_serialize(&obj),
            "n_xmldoc" => pfw::xml_serialize(&obj),
            cls @ _ => panic!("unexpect class {cls}")
        };
        let msg = if retain.unwrap_or_default() {
            Message::new_retained(topic.clone(), data, qos.unwrap_or_default())
        } else {
            Message::new(topic.clone(), data, qos.unwrap_or_default())
        };
        self.conn_builder.will_message(msg);
        self
    }
}

#[derive(Default)]
struct RuntimeStore {
    map: HashMap<String, Vec<u8>>
}

#[allow(unused_variables)]
impl ClientPersistence for RuntimeStore {
    fn open(&mut self, client_id: &str, server_uri: &str) -> paho_mqtt::Result<()> { Ok(()) }
    fn close(&mut self) -> paho_mqtt::Result<()> { Ok(()) }
    fn put(&mut self, key: &str, buffers: Vec<&[u8]>) -> paho_mqtt::Result<()> {
        self.map.insert(
            key.to_owned(),
            buffers.into_iter().fold(Vec::new(), |mut buf, item| {
                buf.extend_from_slice(item);
                buf
            })
        );
        Ok(())
    }
    fn get(&mut self, key: &str) -> paho_mqtt::Result<Vec<u8>> {
        Ok(self.map.get(key).cloned().unwrap_or_default())
    }
    fn remove(&mut self, key: &str) -> paho_mqtt::Result<()> {
        self.map.remove(key);
        Ok(())
    }
    fn keys(&mut self) -> paho_mqtt::Result<Vec<String>> { Ok(self.map.keys().cloned().collect()) }
    fn clear(&mut self) -> paho_mqtt::Result<()> {
        self.map.clear();
        Ok(())
    }
    fn contains_key(&mut self, key: &str) -> bool { self.map.contains_key(key) }
}
