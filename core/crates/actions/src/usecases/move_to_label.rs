//! V: move to a label = label + archive, ONE action with ONE undo entry (contracts delta 10, addition 13).
use crate::engine::{core_first, load, Applied, Env};
use crate::usecases::{archive, label};
use contracts::{CommandOutcome, CoreError, UndoAction};

pub struct MoveToLabel {
    env: Env,
}

impl MoveToLabel {
    pub fn new(env: Env) -> MoveToLabel {
        MoveToLabel { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, label_id: String) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, now| {
            let Some(target) = ctx.label_get(&label_id)? else { return Ok(Err(CoreError::NotFound { kind: "label".into(), id: label_id.clone() })) };
            let rows = match load(ctx, &message_ids)? {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            let carrying = ctx.label_carriers(&label_id, &message_ids, true)?;
            let labelled: Vec<String> = message_ids.iter().filter(|i| !carrying.contains(i)).cloned().collect();
            let archived: Vec<String> = rows.iter().filter(|r| !r.archived).map(|r| r.id.clone()).collect();
            let a = match label::assign(ctx, &rows, &target.path, false, now, false)? {
                Ok(a) => a,
                Err(e) => return Ok(Err(e)),
            };
            let b = match archive::apply(ctx, &message_ids, true, false)? {
                Ok(b) => b,
                Err(e) => return Ok(Err(e)),
            };
            let mut both = Applied::merge(a, b);
            if !labelled.is_empty() || !archived.is_empty() {
                both.undo = Some(UndoAction::MoveToLabel { label_id: label_id.clone(), labelled, archived });
            }
            Ok(Ok(both))
        })
    }
}
