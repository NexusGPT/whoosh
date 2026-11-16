//! Drains the push-job queue. The job row is already written (ledger first); this sends and then
//! records the result. Delivery is at-least-once: a crash between send and record resends on
//! restart, and the collapse id keeps that to one notification on the phone.
use super::{ApnsState, SendOutcome};
use crate::registry::{devices, jobs, now_unix};
use crate::routes::AppState;
use tracing::{error, info, warn};

pub async fn run(state: AppState) {
    loop {
        let ApnsState::Ready(sender) = state.apns.as_ref() else {
            state.dispatcher.idle().await;
            continue;
        };
        match jobs::next_queued(&state.store) {
            Ok(Some(job)) => {
                let (new_state, status, reason) = match sender.send(&job).await {
                    Ok(SendOutcome { status: 200, .. }) => ("sent", Some(200), None),
                    Ok(SendOutcome { status, reason, body }) => {
                        let gone = status == 410 || reason.as_deref() == Some("BadDeviceToken") || reason.as_deref() == Some("Unregistered");
                        if gone {
                            info!("apns says token is gone ({status} {reason:?}); removing the registration");
                            if let Err(e) = devices::unregister(&state.store, &job.device_token, &job.routing_id) {
                                error!("could not remove gone registration: {e}");
                            }
                        } else {
                            warn!("apns refused job {}: status {status}, body {body}", job.job_id);
                        }
                        (if gone { "rejected" } else { "failed" }, Some(status), reason)
                    }
                    Err(e) => {
                        warn!("apns send failed for job {}: {e}", job.job_id);
                        ("failed", None, Some(e))
                    }
                };
                if let Err(e) = jobs::finish(&state.store, job.job_id, new_state, status, reason.as_deref(), now_unix()) {
                    error!("could not record job {} result: {e}", job.job_id);
                    state.dispatcher.idle().await;
                }
            }
            Ok(None) => state.dispatcher.idle().await,
            Err(e) => {
                error!("queue read failed: {e}");
                state.dispatcher.idle().await;
            }
        }
    }
}
