use std::any::Any;
use std::sync::Arc;
use std::time::{Instant};
use quinn_proto::congestion::{Controller, ControllerFactory};
use quinn_proto::RttEstimator;

/// A congestion controller to use when setting a maximum sending rate.
///
/// The maximum sending rate is specified in quinn's transport options (currently only available in
/// our fork of quinn). Since the sending rate is handled there, this congestion controller sets the
/// congestion window to the maximum value (thereby delegating congestion control to the
/// rate-limiting logic inside quinn).
#[derive(Clone)]
pub struct RateLimitBasedCC;

const CONGESTION_WINDOW: u64 = u64::MAX;

impl Controller for RateLimitBasedCC {
    fn on_ack(
        &mut self,
        _now: Instant,
        _sent: Instant,
        _bytes: u64,
        _app_limited: bool,
        _rtt: &RttEstimator,
    ) {}

    fn on_congestion_event(
        &mut self,
        _now: Instant,
        _sent: Instant,
        _is_persistent_congestion: bool,
        _lost_bytes: u64,
    ) {}

    fn on_mtu_update(&mut self, _new_mtu: u16) {}

    fn window(&self) -> u64 {
        CONGESTION_WINDOW
    }

    fn clone_box(&self) -> Box<dyn Controller> {
        Box::new(self.clone())
    }

    fn initial_window(&self) -> u64 {
        CONGESTION_WINDOW
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub struct RateLimitBasedCCConfig;

impl ControllerFactory for RateLimitBasedCCConfig {
    fn build(self: Arc<Self>, _now: Instant, _current_mtu: u16) -> Box<dyn Controller> {
        Box::new(RateLimitBasedCC)
    }
}
