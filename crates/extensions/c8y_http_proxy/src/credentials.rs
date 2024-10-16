use async_trait::async_trait;
use c8y_api::http_proxy::C8yMqttJwtTokenRetriever;
use c8y_api::http_proxy::JwtError;
use tedge_actors::ClientMessageBox;
use tedge_actors::Sequential;
use tedge_actors::Server;
use tedge_actors::ServerActorBuilder;
use tedge_actors::ServerConfig;
use tedge_config::TopicPrefix;

pub type AuthRequest = ();
pub type AuthResult = Result<String, JwtError>;

/// Retrieves Authorization header value authenticating the device
pub type AuthRetriever = ClientMessageBox<AuthRequest, AuthResult>;

/// A JwtRetriever that gets JWT tokens from C8Y over MQTT
pub struct C8YJwtRetriever {
    mqtt_retriever: C8yMqttJwtTokenRetriever,
}

impl C8YJwtRetriever {
    pub fn builder(
        mqtt_config: mqtt_channel::Config,
        topic_prefix: TopicPrefix,
    ) -> ServerActorBuilder<C8YJwtRetriever, Sequential> {
        let mqtt_retriever = C8yMqttJwtTokenRetriever::new(mqtt_config, topic_prefix);
        let server = C8YJwtRetriever { mqtt_retriever };
        ServerActorBuilder::new(server, &ServerConfig::default(), Sequential)
    }
}

#[async_trait]
impl Server for C8YJwtRetriever {
    type Request = AuthRequest;
    type Response = AuthResult;

    fn name(&self) -> &str {
        "C8YJwtRetriever"
    }

    async fn handle(&mut self, _request: Self::Request) -> Self::Response {
        let response = self.mqtt_retriever.get_jwt_token().await?;
        let auth_value = format!("Bearer {}", response.token());
        Ok(auth_value)
    }
}

/// A JwtRetriever that simply always returns the same JWT token (possibly none)
#[cfg(test)]
pub(crate) struct ConstJwtRetriever {
    pub token: String,
}

#[async_trait]
#[cfg(test)]
impl Server for ConstJwtRetriever {
    type Request = AuthRequest;
    type Response = AuthResult;

    fn name(&self) -> &str {
        "ConstJwtRetriever"
    }

    async fn handle(&mut self, _request: Self::Request) -> Self::Response {
        let auth_value = format!("Bearer {}", self.token);
        Ok(auth_value)
    }
}
