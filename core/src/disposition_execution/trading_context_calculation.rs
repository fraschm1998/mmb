use mmb_utils::DateTime;

use crate::disposition_execution::TradingContext;
use crate::explanation::Explanation;
use crate::order_book::local_snapshot_service::LocalSnapshotsService;
use crate::strategies::disposition_strategy::DispositionStrategy;

pub fn calculate_trading_context(
    strategy: &mut dyn DispositionStrategy,
    local_snapshots_service: &LocalSnapshotsService,
    now: DateTime,
) -> Option<TradingContext> {
    // TODO check is balance manager initialized for next calculations

    let mut explanation = Explanation::default();
    explanation.add_reason(format!("Start time utc={}", now.to_rfc2822()));

    // TODO check balance position

    strategy.calculate_trading_context(now, local_snapshots_service, &mut explanation)
}
