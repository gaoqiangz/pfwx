use super::*;
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

    #[method(name = "GetDataString")]
    fn payload_str(&self) -> Cow<str> { self.inner.as_ref().map(|msg| msg.payload_str()).unwrap_or_default() }
}
