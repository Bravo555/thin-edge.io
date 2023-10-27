use std::{collections::HashMap, fmt::Debug, sync::Arc};

use async_trait::async_trait;
use mqtt_channel::StreamExt;
use tedge_actors::{futures::channel::mpsc, Actor, DynSender, RuntimeError, RuntimeRequest};
use tedge_api::{
    entity_store::{EntityRegistrationMessage, EntityType},
    mqtt_topics::{ChannelFilter, EntityFilter, EntityTopicId, MqttSchema},
};
use tedge_mqtt_ext::MqttMessage;
use tracing::{instrument, warn};

use tedge_api::ThinEdgeMessage;

/// An MQTT actor used by the entities to send thin-edge.io messages.
pub struct EntityMqttActor {
    pub(super) device_topic_id: EntityTopicId,
    pub(super) service_topic_id: EntityTopicId,
    pub(super) service_type: String,

    pub(super) mqtt_schema: MqttSchema,

    pub(super) signal_receiver: mpsc::Receiver<RuntimeRequest>,

    pub(super) thin_edge_message_receiver: mpsc::Receiver<ThinEdgeMessage>,
    pub(super) actors: HashMap<ChannelFilter, DynSender<ThinEdgeMessage>>,

    pub(super) mqtt_message_channel: (DynSender<MqttMessage>, mpsc::Receiver<MqttMessage>),
}

impl Debug for EntityMqttActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[async_trait]
impl Actor for EntityMqttActor {
    fn name(&self) -> &str {
        "EntityMqttActor"
    }

    async fn run(mut self) -> Result<(), RuntimeError> {
        let registration_message = EntityRegistrationMessage {
            topic_id: self.service_topic_id.clone(),
            external_id: None,
            r#type: EntityType::Service,
            parent: Some(self.device_topic_id.clone()),
            other: [("type".to_string(), serde_json::json!(self.service_type))]
                .into_iter()
                .collect(),
        };

        let registration_message = registration_message.to_mqtt_message(&self.mqtt_schema);
        self.mqtt_message_channel
            .0
            .send(registration_message)
            .await?;

        loop {
            tokio::select! {
                request = self.signal_receiver.next() => {
                    if let Some(RuntimeRequest::Shutdown) = request {
                        break;
                    }
                }

                Some(thin_edge_message) = self.thin_edge_message_receiver.next() => {
                    self.handle_thin_edge_message(thin_edge_message).await?
                }

                Some(mqtt_message) = self.mqtt_message_channel.1.next() => {
                    self.handle_mqtt_message(mqtt_message).await?
                }
            }
        }

        todo!()
    }
}

impl EntityMqttActor {
    async fn handle_thin_edge_message(
        &mut self,
        message: ThinEdgeMessage,
    ) -> Result<(), RuntimeError> {
        let topic = self
            .mqtt_schema
            .topic_for(&message.entity, &message.channel);

        let mqtt_message = MqttMessage::new(&topic, message.payload.to_vec());

        self.mqtt_message_channel
            .0
            .send(mqtt_message)
            .await
            .unwrap();

        Ok(())
    }

    async fn handle_mqtt_message(&mut self, message: MqttMessage) -> Result<(), RuntimeError> {
        let Ok((entity, channel)) = self.mqtt_schema.entity_channel_of(&message.topic) else {
            return Ok(());
        };

        let thin_edge_message = ThinEdgeMessage {
            entity,
            channel,
            payload: Arc::from(message.payload.0),
        };

        // a service can respond to messages to that service, but also to device directly
        let service_filter = EntityFilter::Entity(&self.service_topic_id);
        let device_filter = EntityFilter::Entity(&self.device_topic_id);

        // AFAIK we don't have any channels that are the same for device and service topic ids, so
        // we will send a message to an actor if that message is to either device or service, and
        // rely on ChannelFilter to filter out unintended messages
        for (channel_filter, actor) in self.actors.iter_mut() {
            let service_topic_filter = self
                .mqtt_schema
                .topics(service_filter.clone(), channel_filter.clone());

            let device_topic_filter = self
                .mqtt_schema
                .topics(device_filter.clone(), channel_filter.clone());

            if service_topic_filter.accept_topic(&message.topic)
                || device_topic_filter.accept_topic(&message.topic)
            {
                actor.send(thin_edge_message.clone()).await?;
            }
        }

        Ok(())
    }
}
