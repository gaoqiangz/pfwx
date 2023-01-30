use super::*;
use crate::base::{conv, pfw};
use paho_mqtt::Message;
use std::borrow::Cow;

#[derive(Default)]
pub struct MqttMessage {
    inner: Option<Message>
}

#[nonvisualobject(name = "nx_mqttmessage")]
impl MqttMessage {
    pub fn init(&mut self, msg: Message) { self.inner = Some(msg); }

    #[method(name = "IsValid")]
    fn is_valid(&self) -> bool { self.inner.is_some() }

    #[method(name = "IsRetained")]
    fn is_retained(&self) -> bool { self.inner.as_ref().map(|msg| msg.retained()).unwrap_or_default() }

    #[method(name = "GetTopic")]
    fn topic(&self) -> &str { self.inner.as_ref().map(|msg| msg.topic()).unwrap_or_default() }

    #[method(name = "GetQoS")]
    fn qos(&self) -> pblong { self.inner.as_ref().map(|msg| msg.qos()).unwrap_or_default() }

    #[method(name = "GetData")]
    fn payload_binary(&self) -> &[u8] { self.inner.as_ref().map(|msg| msg.payload()).unwrap_or_default() }

    #[method(name = "GetDataString", overload = 1)]
    fn payload_str(&self, encoding: Option<pblong>) -> Cow<str> {
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
