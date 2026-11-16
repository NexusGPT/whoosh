//! List-Unsubscribe, one click (RFC 8058). The server is asked first, because an unsubscribe cannot be taken back; only after
//! it said yes the message gets the system label UNSUBSCRIBED_LABEL and leaves the inbox. A refusal changes nothing.
use super::{archive, label};
use crate::constants::UNSUBSCRIBED_LABEL;
use crate::engine::{load, local_only, Applied, Env};
use crate::port::WriteBack;
use contracts::CoreError;

pub struct Unsubscribe {
    env: Env,
}

impl Unsubscribe {
    pub fn new(env: Env) -> Unsubscribe {
        Unsubscribe { env }
    }

    pub fn execute(&self, message_id: String) -> Result<(), CoreError> {
        let id = message_id.clone();
        let found = self.env.store.write(move |ctx| {
            let row = match load(ctx, std::slice::from_ref(&id))? {
                Ok(mut r) => r.remove(0),
                Err(e) => return Ok(Err(e)),
            };
            let listed = ctx.has_list_header(&id)?;
            Ok(Ok((row.account_id.clone(), row.as_ref(), listed)))
        })??;
        let (account_id, message, listed) = found;
        if !listed {
            return Err(CoreError::Invalid { field: "message_id".into(), reason_words: "this message has no unsubscribe link".into() });
        }
        self.env.adapter.execute(WriteBack::Unsubscribe { account_id, message }).map_err(|r| self.env.failure(&r))?;
        local_only(&self.env, move |ctx, now| {
            let ids = vec![message_id];
            let rows = match load(ctx, &ids)? {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            if let Err(e) = label::assign(ctx, &rows, UNSUBSCRIBED_LABEL, true, now, false)? {
                return Ok(Err(e));
            }
            if let Err(e) = archive::apply(ctx, &ids, true, false)? {
                return Ok(Err(e));
            }
            Ok(Ok(Applied::nothing()))
        })
    }
}
