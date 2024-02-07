use crate::actor::TimerActor;
use crate::AnyPayload;
use crate::SetTimeout;
use crate::Timeout;
use async_trait::async_trait;
use std::convert::Infallible;
use tedge_actors::Builder;
use tedge_actors::ChannelError;
use tedge_actors::CloneSender;
use tedge_actors::DynSender;
use tedge_actors::Message;
use tedge_actors::NoConfig;
use tedge_actors::RuntimeRequest;
use tedge_actors::RuntimeRequestSink;
use tedge_actors::Sender;
use tedge_actors::ServerMessageBoxBuilder;
use tedge_actors::ServiceProvider;

pub struct TimerActorBuilder {
    box_builder: ServerMessageBoxBuilder<SetTimeout<AnyPayload>, Timeout<AnyPayload>>,
}

impl Default for TimerActorBuilder {
    fn default() -> Self {
        TimerActorBuilder {
            box_builder: ServerMessageBoxBuilder::new("Timer", 16),
        }
    }
}

impl Builder<TimerActor> for TimerActorBuilder {
    type Error = Infallible;

    fn try_build(self) -> Result<TimerActor, Self::Error> {
        Ok(self.build())
    }

    fn build(self) -> TimerActor {
        let actor_box = self.box_builder.build();
        TimerActor::new(actor_box)
    }
}

impl RuntimeRequestSink for TimerActorBuilder {
    fn get_signal_sender(&self) -> DynSender<RuntimeRequest> {
        self.box_builder.get_signal_sender()
    }
}

impl<T: Message> ServiceProvider<SetTimeout<T>, Timeout<T>, NoConfig> for TimerActorBuilder {
    fn connect_consumer(
        &mut self,
        config: NoConfig,
        response_sender: DynSender<Timeout<T>>,
    ) -> DynSender<SetTimeout<T>> {
        let adapted_response_sender = Box::new(TimeoutSender {
            inner: response_sender,
        });
        let request_sender = self
            .box_builder
            .connect_consumer(config, adapted_response_sender);
        Box::new(SetTimeoutSender {
            inner: request_sender,
        })
    }
}

/// A Sender that translates timeout responses on the wire
///
/// This sender receives `Timeout<AnyPayload>` from the `TimerActor`,
/// and translates then forwards these messages to an actor expecting `Timeout<T>`
struct TimeoutSender<T: Message> {
    inner: DynSender<Timeout<T>>,
}

impl<T: Message> Clone for TimeoutSender<T> {
    fn clone(&self) -> Self {
        TimeoutSender {
            inner: self.inner.sender_clone(),
        }
    }
}

#[async_trait]
impl<T: Message> Sender<Timeout<AnyPayload>> for TimeoutSender<T> {
    async fn send(&mut self, message: Timeout<AnyPayload>) -> Result<(), ChannelError> {
        if let Ok(event) = message.event.downcast() {
            self.inner.send(Timeout { event: *event }).await?;
        }
        Ok(())
    }
}

/// A Sender that translates timeout requests on the wire
///
/// This sender receives `SetTimeout<T>` requests from some actor,
/// and translates then forwards these messages to the timer actor expecting`Timeout<AnyPayload>`
struct SetTimeoutSender {
    inner: DynSender<SetTimeout<AnyPayload>>,
}

impl Clone for SetTimeoutSender {
    fn clone(&self) -> Self {
        SetTimeoutSender {
            inner: self.inner.sender_clone(),
        }
    }
}

#[async_trait]
impl<T: Message> Sender<SetTimeout<T>> for SetTimeoutSender {
    async fn send(&mut self, request: SetTimeout<T>) -> Result<(), ChannelError> {
        let duration = request.duration;
        let event: AnyPayload = Box::new(request.event);
        self.inner.send(SetTimeout { duration, event }).await
    }
}
