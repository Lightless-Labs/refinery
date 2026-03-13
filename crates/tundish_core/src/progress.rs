use std::sync::Arc;
use std::time::Duration;

use crate::types::ModelId;

/// Simple progress callback for subprocess output.
///
/// Reports `(model, line_count, elapsed)` on each line of subprocess output.
pub type ProgressFn = Arc<dyn Fn(&ModelId, usize, Duration) + Send + Sync>;
