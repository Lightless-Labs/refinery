use std::sync::Arc;
use std::time::Duration;

use crate::types::ModelId;

/// Simple progress callback for subprocess output.
///
/// Reports `(model, line_count, elapsed)` — no knowledge of consensus phases.
pub type ProgressFn = Arc<dyn Fn(&ModelId, usize, Duration) + Send + Sync>;
