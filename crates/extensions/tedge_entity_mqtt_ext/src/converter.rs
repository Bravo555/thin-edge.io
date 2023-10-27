// use tedge_actors::Converter;
// use tedge_api::mqtt_topics::{EntityTopicId, MqttSchema};
// use tedge_mqtt_ext::MqttMessage;

// struct TeMessageMqttConverter {
//     mqtt_schema: MqttSchema,
//     entity_topic_id: EntityTopicId,
// }

// impl Converter for TeMessageMqttConverter {
//     type Input = super::ThinEdgeMessage;

//     type Output = MqttMessage;

//     type Error = TeMessageMqttConverterError;

//     fn convert(&mut self, input: &Self::Input) -> Result<Vec<Self::Output>, Self::Error> {
//         let topic = self
//             .mqtt_schema
//             .topic_for(&self.entity_topic_id, &input.channel);
//         let message = MqttMessage::new(&topic, input.payload.to_owned());

//         Ok(vec![message])
//     }
// }

// #[derive(thiserror::Error, Debug)]
// enum TeMessageMqttConverterError {}
