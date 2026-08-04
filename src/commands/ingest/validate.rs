//! Mode-conditional flag validation for `ingest` (G20).

use super::args::IngestArgs;
use crate::errors::AppError;

/// G20: validate that flags rejected by the selected `--mode` were not
/// passed. Flags silently discarded by the wrong mode are surfaced BEFORE
/// any DB work, so the operator gets an actionable error instead of a
/// surprise at runtime.
///
/// The only surviving mode is `none` (body-only), so nothing is rejected
/// today. The hook stays so a future mode reintroduces its matrix here.
pub(crate) fn validate_mode_conditional_flags_ingest(_args: &IngestArgs) -> Result<(), AppError> {
    Ok(())
}
