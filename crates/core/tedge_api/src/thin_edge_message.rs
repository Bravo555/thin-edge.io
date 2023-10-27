use std::sync::Arc;

use crate::mqtt_topics::{Channel, EntityTopicId};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ThinEdgeMessage {
    pub entity: EntityTopicId,
    pub channel: Channel,
    pub payload: Arc<[u8]>,
}
