use std::collections::HashMap;
use std::net::TcpListener;
use std::time::Duration;

use backoff::backoff::Backoff;
use backoff::exponential::ExponentialBackoff;
use backoff::SystemClock;
use futures::channel::mpsc::UnboundedReceiver;
use once_cell::sync::Lazy;
use rumqttc::Event;
use rumqttc::Incoming;
use rumqttc::QoS;
use rumqttd::Broker;
use rumqttd::Config;
use rumqttd::ConnectionSettings;
use rumqttd::ServerSettings;

static SERVER: Lazy<MqttProcessHandler> = Lazy::new(MqttProcessHandler::new);

pub fn test_mqtt_broker() -> &'static MqttProcessHandler {
    Lazy::force(&SERVER)
}

pub struct MqttProcessHandler {
    pub port: u16,
}

impl MqttProcessHandler {
    pub fn new() -> MqttProcessHandler {
        let mut backoff = ExponentialBackoff::<SystemClock>::default();
        loop {
            if let Ok(port) = std::panic::catch_unwind(spawn_broker) {
                break MqttProcessHandler { port };
            } else {
                std::thread::sleep(backoff.next_backoff().unwrap());
            }
        }
    }

    pub async fn publish(&self, topic: &str, payload: &str) -> Result<(), anyhow::Error> {
        crate::test_mqtt_client::publish(self.port, topic, payload, QoS::AtLeastOnce, false).await
    }

    pub async fn publish_with_opts(
        &self,
        topic: &str,
        payload: &str,
        qos: QoS,
        retain: bool,
    ) -> Result<(), anyhow::Error> {
        crate::test_mqtt_client::publish(self.port, topic, payload, qos, retain).await
    }

    pub async fn messages_published_on(&self, topic: &str) -> UnboundedReceiver<String> {
        crate::test_mqtt_client::messages_published_on(self.port, topic).await
    }

    pub async fn wait_for_response_on_publish(
        &self,
        pub_topic: &str,
        pub_message: &str,
        sub_topic: &str,
        timeout: Duration,
    ) -> Option<String> {
        crate::test_mqtt_client::wait_for_response_on_publish(
            self.port,
            pub_topic,
            pub_message,
            sub_topic,
            timeout,
        )
        .await
    }

    pub fn map_messages_background<F>(&self, func: F)
    where
        F: 'static + Send + Sync + Fn((String, String)) -> Vec<(String, String)>,
    {
        tokio::spawn(crate::test_mqtt_client::map_messages_loop(self.port, func));
    }
}

impl Default for MqttProcessHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn spawn_broker() -> u16 {
    // We can get a free port from the kernel by binding on port 0. We can then
    // immediately drop the listener, and use the port for the mqtt broker.
    // Unfortunately we can run into a race condition whereas when tests are run
    // in parallel, when we get a certain free port, after dropping the listener
    // the port is freed, and `TcpListener::bind` in another test might pick it
    // up before we start the mqtt broker. For this reason, we keep retrying
    // the operation if the port is already in use.
    //
    // This would have been much easier if rumqttd would just let us query the
    // port the server got after we've passed in 0 as the port but currently
    // it's not possible, so we have to rely on this workaround.
    let port = loop {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let config = get_rumqttd_config(port);
        let mut broker = Broker::new(config);
        let (mut tx, _rx) = broker.link("localclient").unwrap();
        tx.subscribe("#").unwrap();

        // `broker.start()` blocks, so to catch a TCP port bind error we have to
        // start it in a thread and wait a bit.
        let broker_thread = std::thread::spawn(move || {
            eprintln!("MQTT-TEST INFO: start test MQTT broker (port = {})", port);
            broker.start()
        });
        std::thread::sleep(std::time::Duration::from_millis(50));

        if !broker_thread.is_finished() {
            break port;
        }

        match broker_thread.join() {
            Ok(Ok(())) => {
                // I don't know why it happened, but I have observed this once while testing
                // So just log the error and retry starting the broker on a new port
                eprintln!("MQTT-TEST ERROR: `broker.start()` should not terminate until after `spawn_broker` returns")
            }
            Ok(Err(err)) => {
                eprintln!(
                    "MQTT-TEST ERROR: fail to start the test MQTT broker: {:?}",
                    err
                );
            }
            Err(err) => {
                eprintln!(
                    "MQTT-TEST ERROR: fail to start the test MQTT broker: {:?}",
                    err
                );
            }
        }
    };

    std::thread::spawn(move || {
        let mut mqttoptions = rumqttc::MqttOptions::new("rumqtt-sync", "127.0.0.1", port);
        mqttoptions.set_keep_alive(Duration::from_secs(5));

        let (client, mut connection) = rumqttc::Client::new(mqttoptions, 10);

        client.subscribe("#", QoS::ExactlyOnce).unwrap();

        loop {
            let msg = connection.recv();
            if let Ok(Ok(Event::Incoming(Incoming::Publish(publish)))) = msg {
                let payload = match std::str::from_utf8(publish.payload.as_ref()) {
                    Ok(payload) => format!("{:.110}", payload),
                    Err(_) => format!("Non uft8 ({} bytes)", publish.payload.len()),
                };
                eprintln!(
                    "MQTT-TEST MSG: topic = {}, payload = {:?}",
                    publish.topic, payload
                );
            }
        }
    });

    port
}

fn get_rumqttd_config(port: u16) -> Config {
    let router_config = rumqttd::RouterConfig {
        max_segment_size: 10240,
        max_segment_count: 10,
        max_connections: 1000,
        initialized_filters: None,
        ..Default::default()
    };

    let connections_settings = ConnectionSettings {
        connection_timeout_ms: 1000,
        max_payload_size: 268435455,
        max_inflight_count: 200,
        auth: None,
        dynamic_filters: false,
        external_auth: None,
    };

    let server_config = ServerSettings {
        name: "1".to_string(),
        listen: ([127, 0, 0, 1], port).into(),
        tls: None,
        next_connection_delay_ms: 1,
        connections: connections_settings,
    };

    let mut servers = HashMap::new();
    servers.insert("1".to_string(), server_config);

    rumqttd::Config {
        id: 0,
        router: router_config,
        cluster: None,
        console: None,
        v4: Some(servers),
        ws: None,
        v5: None,
        bridge: None,
        prometheus: None,
        metrics: None,
    }
}
