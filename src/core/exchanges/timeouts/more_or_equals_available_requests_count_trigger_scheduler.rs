use std::sync::Arc;

use crate::core::{utils::custom_spawn, DateTime};
use anyhow::Result;
use chrono::{Duration, Utc};
use futures::FutureExt;
use log::error;
use parking_lot::Mutex;
use tokio::time::sleep;

pub type TriggerHandler = Mutex<Box<dyn FnMut() -> Result<()> + Send>>;

pub struct MoreOrEqualsAvailableRequestsCountTriggerScheduler {
    increasing_count_triggers: Mutex<Vec<Arc<MoreOrEqualsAvailableRequestsCountTrigger>>>,
}

impl MoreOrEqualsAvailableRequestsCountTriggerScheduler {
    pub fn new() -> Self {
        let triggers = Mutex::new(Vec::new());
        Self {
            increasing_count_triggers: triggers,
        }
    }

    pub fn utc_now() -> DateTime {
        Utc::now()
    }

    pub fn register_trigger(&self, count_threshold: usize, handler: TriggerHandler) {
        let trigger = Arc::new(MoreOrEqualsAvailableRequestsCountTrigger::new(
            count_threshold,
            handler,
        ));
        self.increasing_count_triggers.lock().push(trigger);
    }

    pub fn schedule_triggers(
        &self,
        available_requests_count_on_last_request_time: usize,
        last_request_time: DateTime,
        period_duration: Duration,
    ) {
        let current_time = Self::utc_now();

        for trigger in self.increasing_count_triggers.lock().iter() {
            trigger.clone().schedule_handler(
                available_requests_count_on_last_request_time,
                last_request_time,
                period_duration,
                current_time,
            );
        }
    }
}

struct MoreOrEqualsAvailableRequestsCountTrigger {
    count_threshold: usize,
    handler: TriggerHandler,
}

impl MoreOrEqualsAvailableRequestsCountTrigger {
    fn new(count_threshold: usize, handler: TriggerHandler) -> Self {
        Self {
            count_threshold,
            handler,
        }
    }

    pub fn schedule_handler(
        self: Arc<Self>,
        available_requests_count_on_last_request_time: usize,
        last_request_time: DateTime,
        period_duration: Duration,
        current_time: DateTime,
    ) {
        let is_greater = available_requests_count_on_last_request_time >= self.count_threshold;

        if is_greater {
            return;
        }

        // Note: suppose that requests restriction same as in RequestsTimeoutManager (requests count in specified time period)
        // It logical dependency to RequestsTimeoutManager how calculate trigger time
        // var triggerTime = isGreater ? lastRequestTime : lastRequestTime + periodDuration;
        let trigger_time = last_request_time + period_duration;
        let mut delay = trigger_time - current_time;
        delay = delay.max(Duration::zero());

        let action = async move {
            self.clone().handle_inner(delay).await;
            Ok(())
        };
        custom_spawn("handle_inner for schedule_handler()", true, action.boxed());
    }

    async fn handle_inner(&self, delay: Duration) {
        match delay.to_std() {
            Ok(delay) => {
                sleep(delay).await;
                if let Err(error) = (*self.handler.lock())() {
                    error!(
                        "Error in MoreOrEqualsAvailableRequestsCountTrigger: {}",
                        error
                    );
                }
            }
            Err(error) => {
                error!(
                    "Unable to convert chrono::Duration to std::Duration: {}",
                    error
                );
            }
        }
    }
}
