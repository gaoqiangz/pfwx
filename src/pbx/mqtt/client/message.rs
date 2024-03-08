use super::*;
use crate::base::{conv, pfw};
use paho_mqtt::MessageBuilder;
use std::borrow::Cow;

#[derive(Default)]
pub struct MqttMessage {
    inner: Option<Message>
}

#[nonvisualobject(name = "nx_mqttmessage")]
impl MqttMessage {
    pub fn init(&mut self, msg: Message) { self.inner = Some(msg); }

    /// 获取`paho_mqtt::Message`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn take(&mut self) -> Option<Message> { self.inner.take() }

    #[method(name = "IsValid")]
    fn is_valid(&self) -> bool { self.inner.is_some() }

    #[method(name = "SetRetained")]
    fn set_retained(&mut self, retain: bool) -> RetCode {
        self.inner = match self.inner.take() {
            Some(msg) => {
                Some(if retain {
                    Message::new_retained(msg.topic(), msg.payload(), msg.qos())
                } else {
                    Message::new(msg.topic(), msg.payload(), msg.qos())
                })
            },
            None => Some(MessageBuilder::new().retained(retain).finalize())
        };
        RetCode::OK
    }

    #[method(name = "IsRetained")]
    fn is_retained(&self) -> bool { self.inner.as_ref().map(|msg| msg.retained()).unwrap_or_default() }

    #[method(name = "SetTopic")]
    fn set_topic(&mut self, topic: String) -> RetCode {
        self.inner = match self.inner.take() {
            Some(msg) => {
                Some(if msg.retained() {
                    Message::new_retained(topic, msg.payload(), msg.qos())
                } else {
                    Message::new(topic, msg.payload(), msg.qos())
                })
            },
            None => Some(MessageBuilder::new().topic(topic).finalize())
        };
        RetCode::OK
    }

    #[method(name = "GetTopic")]
    fn topic(&self) -> &str { self.inner.as_ref().map(|msg| msg.topic()).unwrap_or_default() }

    #[method(name = "SetQoS")]
    fn set_qos(&mut self, qos: pblong) -> RetCode {
        self.inner = match self.inner.take() {
            Some(msg) => {
                Some(if msg.retained() {
                    Message::new_retained(msg.topic(), msg.payload(), qos)
                } else {
                    Message::new(msg.topic(), msg.payload(), qos)
                })
            },
            None => Some(MessageBuilder::new().qos(qos).finalize())
        };
        RetCode::OK
    }

    #[method(name = "GetQoS")]
    fn qos(&self) -> pblong { self.inner.as_ref().map(|msg| msg.qos()).unwrap_or_default() }

    #[method(name = "SetData")]
    fn set_payload_binary(&mut self, data: &[u8]) -> RetCode {
        self.inner = match self.inner.take() {
            Some(msg) => {
                Some(if msg.retained() {
                    Message::new_retained(msg.topic(), data, msg.qos())
                } else {
                    Message::new(msg.topic(), data, msg.qos())
                })
            },
            None => Some(MessageBuilder::new().payload(data).finalize())
        };
        RetCode::OK
    }

    #[method(name = "SetData", overload = 1)]
    fn set_payload_string(&mut self, data: String, encoding: Option<pblong>) -> RetCode {
        let data = conv::encode(&data, encoding.unwrap_or(conv::ENCODING_UTF8));
        self.inner = match self.inner.take() {
            Some(msg) => {
                Some(if msg.retained() {
                    Message::new_retained(msg.topic(), data, msg.qos())
                } else {
                    Message::new(msg.topic(), data, msg.qos())
                })
            },
            None => Some(MessageBuilder::new().payload(data).finalize())
        };
        RetCode::OK
    }

    #[method(name = "SetData")]
    fn set_payload_json_or_xml(&mut self, obj: Object) -> RetCode {
        let data = match obj.get_class_name().as_str() {
            "n_json" => pfw::json_serialize(&obj),
            "n_xmldoc" => pfw::xml_serialize(&obj),
            cls @ _ => panic!("unexpect class {cls}")
        };
        self.inner = match self.inner.take() {
            Some(msg) => {
                Some(if msg.retained() {
                    Message::new_retained(msg.topic(), data, msg.qos())
                } else {
                    Message::new(msg.topic(), data, msg.qos())
                })
            },
            None => Some(MessageBuilder::new().payload(data).finalize())
        };
        RetCode::OK
    }

    #[method(name = "GetData")]
    fn payload_binary(&self) -> &[u8] { self.inner.as_ref().map(|msg| msg.payload()).unwrap_or_default() }

    #[method(name = "GetDataString", overload = 1)]
    fn payload_string(&self, encoding: Option<pblong>) -> Cow<str> {
        if let Some(data) = self.inner.as_ref().map(|msg| msg.payload()) {
            conv::decode(&data, encoding.unwrap_or(conv::ENCODING_UTF8))
        } else {
            "".into()
        }
    }

    #[method(name = "GetDataJSON", overload = 1)]
    fn payload_json(&self, encoding: Option<pblong>) -> Object {
        let data = if let Some(data) = self.inner.as_ref().map(|msg| msg.payload()) {
            conv::decode(&data, encoding.unwrap_or(conv::ENCODING_UTF8))
        } else {
            "".into()
        };
        pfw::json_parse(self.get_session(), &data)
    }

    #[method(name = "GetDataXML", overload = 1)]
    fn payload_xml(&self, encoding: Option<pblong>) -> Object {
        let data = if let Some(data) = self.inner.as_ref().map(|msg| msg.payload()) {
            conv::decode(&data, encoding.unwrap_or(conv::ENCODING_UTF8))
        } else {
            "".into()
        };
        pfw::xml_parse(self.get_session(), &data)
    }
}
