//! Native editable project file (`.osd`, JSON). The codec lives in
//! [`openstitch_design::project`]; this wraps its errors for the commands.

use crate::params::CommandError;
use openstitch_design::Design;
pub use openstitch_design::project::{EXTENSION, FORMAT, MAX_SIZE, VERSION};

pub fn encode(design: &Design) -> Result<Vec<u8>, CommandError> {
    openstitch_design::project::encode(design).map_err(|reason| CommandError::Failed {
        command: "project.save".into(),
        reason,
    })
}

pub fn decode(data: &[u8]) -> Result<Design, CommandError> {
    let design =
        openstitch_design::project::decode(data).map_err(|reason| CommandError::Failed {
            command: "project.open".into(),
            reason,
        })?;
    validate_engine(&design)?;
    Ok(design)
}

fn fail(reason: impl Into<String>) -> CommandError {
    CommandError::Failed {
        command: "project.open".into(),
        reason: reason.into(),
    }
}

/// Full validation of a design from an untrusted source (files, templates): the codec's checks
/// plus the engine-side limits.
pub(crate) fn validate(d: &Design) -> Result<(), CommandError> {
    openstitch_design::project::validate(d).map_err(fail)?;
    validate_engine(d)
}

/// Checks that depend on engine-side limits (sewing stitch patterns, user patterns); the codec
/// in `openstitch_design::project` has already validated objects, coordinates, text and fonts.
fn validate_engine(d: &Design) -> Result<(), CommandError> {
    if d.sewing.len() > crate::sewing_cmds::MAX_SEWING {
        return Err(fail("too many sewing stitch patterns"));
    }
    for (i, q) in d.sewing.iter().enumerate() {
        if d.sewing[..i].iter().any(|o| o.id == q.id) {
            return Err(fail(format!("duplicate sewing stitch id '{}'", q.id)));
        }
    }
    if d.patterns.len() > crate::pattern_cmds::MAX_USER_PATTERNS {
        return Err(fail("too many user patterns"));
    }
    for (i, p) in d.patterns.iter().enumerate() {
        if openstitch_design::patterns::is_bundled(&p.id)
            || d.patterns[..i]
                .iter()
                .any(|q| q.kind == p.kind && q.id == p.id)
        {
            return Err(fail(format!("duplicate or reserved pattern id '{}'", p.id)));
        }
    }
    Ok(())
}
