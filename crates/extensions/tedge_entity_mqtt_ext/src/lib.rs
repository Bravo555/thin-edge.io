//! A thin-edge.io entity-aware MQTT actor.
//!
//! This actor is to be used by thin-edge.io services to send thin-edge.io
//! messages via MQTT. For a service to be able to send an MQTT message, it
//! needs to know 2 things, which are configurable and can easily change:
//!
//! - MQTT topic root
//! - its own MQTT topic ID
//!
//! These options are read and handled by this actor, so that by using it, the
//! caller doesn't need to concern themselves with this state.
//!
//! At the same time, services may need to send messages which are not thin-edge
//! messages (i.e. not starting with MQTT topic root or which don't fit the
//! public thin-edge.io API, e.g. mappers) so we still need the original MQTT
//! actor.

mod actor;
mod converter;

pub use actor::EntityMqttActor;

use std::{collections::HashMap, sync::Arc};

use mqtt_channel::TopicFilter;
use tedge_actors::{
    futures::channel::mpsc, Builder, DynSender, RuntimeRequest, RuntimeRequestSink, ServiceProvider,
};
use tedge_api::{
    mqtt_topics::{Channel, ChannelFilter, EntityTopicError, EntityTopicId, MqttSchema},
    ThinEdgeMessage,
};
use tedge_config::TEdgeConfigReaderMqtt;
use tedge_mqtt_ext::MqttMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MqttConfig {
    pub topic_root: Arc<str>,
    pub device_topic_id: EntityTopicId,
}

impl TryFrom<&TEdgeConfigReaderMqtt> for MqttConfig {
    type Error = EntityTopicError;

    fn try_from(reader: &TEdgeConfigReaderMqtt) -> Result<Self, Self::Error> {
        Ok(Self {
            topic_root: reader.topic_root.as_str().into(),
            device_topic_id: reader.device_topic_id.parse()?,
        })
    }
}

impl EntityMqttActor {
    pub fn builder(
        mqtt_config: MqttConfig,
        mqtt: &mut impl ServiceProvider<MqttMessage, MqttMessage, TopicFilter>,
        service_topic_id: EntityTopicId,
        service_type: String,
    ) -> EntityMqttActorBuilder {
        let MqttConfig {
            topic_root,
            device_topic_id,
        } = mqtt_config;

        let signal_channel = mpsc::channel(10);

        let thin_edge_message_channel = mpsc::channel(10);
        let (mqtt_message_sender, mqtt_message_receiver) = mpsc::channel::<MqttMessage>(10);

        let topic_filter = TopicFilter::new_unchecked(&format!("{topic_root}/#"));

        let mqtt_message_sender =
            mqtt.connect_consumer(topic_filter, mqtt_message_sender.clone().into());

        EntityMqttActorBuilder {
            topic_root,
            device_topic_id,
            service_topic_id,
            service_type,

            signal_channel,
            entities: HashMap::new(),

            thin_edge_message_channel,
            mqtt_message_channel: (mqtt_message_sender, mqtt_message_receiver),
        }
    }
}

pub struct EntityMqttActorBuilder {
    topic_root: Arc<str>,
    device_topic_id: EntityTopicId,
    service_topic_id: EntityTopicId,
    service_type: String,

    signal_channel: (mpsc::Sender<RuntimeRequest>, mpsc::Receiver<RuntimeRequest>),

    thin_edge_message_channel: (
        mpsc::Sender<ThinEdgeMessage>,
        mpsc::Receiver<ThinEdgeMessage>,
    ),
    entities: HashMap<ChannelFilter, DynSender<ThinEdgeMessage>>,

    mqtt_message_channel: (DynSender<MqttMessage>, mpsc::Receiver<MqttMessage>),
}

impl Builder<EntityMqttActor> for EntityMqttActorBuilder {
    type Error = EntityMqttActorError;

    fn try_build(self) -> Result<EntityMqttActor, Self::Error> {
        let mqtt_schema = MqttSchema::with_root(self.topic_root.to_string());

        Ok(EntityMqttActor {
            mqtt_schema,
            device_topic_id: self.device_topic_id,
            service_topic_id: self.service_topic_id,
            service_type: self.service_type,

            signal_receiver: self.signal_channel.1,

            thin_edge_message_receiver: self.thin_edge_message_channel.1,
            actors: self.entities,

            mqtt_message_channel: self.mqtt_message_channel,
        })
    }
}

impl RuntimeRequestSink for EntityMqttActorBuilder {
    fn get_signal_sender(&self) -> tedge_actors::DynSender<tedge_actors::RuntimeRequest> {
        Box::new(self.signal_channel.0.clone())
    }
}

impl ServiceProvider<ThinEdgeMessage, ThinEdgeMessage, ChannelFilter> for EntityMqttActorBuilder {
    fn connect_consumer(
        &mut self,
        config: ChannelFilter,
        response_sender: DynSender<ThinEdgeMessage>,
    ) -> DynSender<ThinEdgeMessage> {
        self.entities.insert(config, response_sender);
        self.thin_edge_message_channel.0.clone().into()
    }
}

#[derive(thiserror::Error, Debug)]
#[error("EntityMqttActorError")]
pub struct EntityMqttActorError {
    #[from]
    source: Option<anyhow::Error>,
}
