#![deny(
    non_ascii_idents,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_must_use,
    clippy::unwrap_used
)]

pub mod events;

use anyhow::{Context, Error, Result};
use binance::binance::BinanceBuilder;
use mmb_core::lifecycle::app_lifetime_manager::ActionAfterGracefulShutdown;
use std::sync::Arc;

use mmb_core::config::{CONFIG_PATH, CREDENTIALS_PATH};
use mmb_core::database::events::transaction::{
    transaction_service, TransactionSnapshot, TransactionStatus, TransactionTrade,
};
use mmb_core::exchanges::common::MarketAccountId;
use mmb_core::exchanges::events::ExchangeEvent;
use mmb_core::infrastructure::spawn_future;
use mmb_core::lifecycle::launcher::{launch_trading_engine, EngineBuildConfig, InitSettings};
use mmb_core::lifecycle::trading_engine::EngineContext;
use mmb_core::order_book::local_snapshot_service::LocalSnapshotsService;
use mmb_core::orders::event::OrderEventType;
use mmb_core::orders::order::OrderSnapshot;
use mmb_core::settings::BaseStrategySettings;
use mmb_utils::infrastructure::{SpawnFutureFlags, WithExpect};

use crate::events::create_liquidity_order_book_snapshot;
use strategies::example_strategy::{ExampleStrategy, ExampleStrategySettings};

const STRATEGY_NAME: &str = "binance_demo";

#[tokio::main]
async fn main() -> Result<()> {
    let engine_config = EngineBuildConfig::new(vec![Box::new(BinanceBuilder)]);

    let init_settings = InitSettings::<ExampleStrategySettings>::Load {
        config_path: CONFIG_PATH.to_owned(),
        credentials_path: CREDENTIALS_PATH.to_owned(),
    };
    loop {
        let engine =
            launch_trading_engine(&engine_config, init_settings.clone(), |settings, ctx| {
                spawn_future(
                    "Save order books",
                    SpawnFutureFlags::STOP_BY_TOKEN | SpawnFutureFlags::DENY_CANCELLATION,
                    start_liquidity_order_book_saving(ctx.clone()),
                );

                Box::new(ExampleStrategy::new(
                    settings.strategy.exchange_account_id(),
                    settings.strategy.currency_pair(),
                    settings.strategy.spread,
                    settings.strategy.max_amount,
                    ctx,
                ))
            })
            .await?;

        match engine.run().await {
            ActionAfterGracefulShutdown::Nothing => break,
            ActionAfterGracefulShutdown::Restart => continue,
        }
    }
    Ok(())
}

async fn start_liquidity_order_book_saving(ctx: Arc<EngineContext>) -> Result<(), Error> {
    let mut snapshots_service = LocalSnapshotsService::default();
    let mut events_rx = ctx.get_events_channel();

    let stop_token = ctx.lifetime_manager.stop_token();
    while !stop_token.is_cancellation_requested() {
        let event_res = events_rx.recv().await;
        match event_res {
            Err(err) => eprintln!("Error occurred: {err:?}"),
            Ok(event) => {
                let market_account_id = match event {
                    ExchangeEvent::OrderBookEvent(ob_event) => snapshots_service.update(ob_event),
                    ExchangeEvent::OrderEvent(order_event) => match order_event.event_type {
                        OrderEventType::CreateOrderSucceeded
                        | OrderEventType::OrderCompleted { .. }
                        | OrderEventType::CancelOrderSucceeded => {
                            Some(order_event.order.fn_ref(|o| o.market_account_id()))
                        }
                        OrderEventType::OrderFilled { cloned_order } => {
                            save_transaction(
                                &ctx,
                                &cloned_order,
                                TransactionStatus::Finished,
                                STRATEGY_NAME.to_string(),
                            )
                            .context("in start_liquidity_order_book_saving")?;

                            Some(cloned_order.market_account_id())
                        }
                        _ => None,
                    },
                    _ => None,
                };

                save_liquidity_order_book_if_can(&ctx, &mut snapshots_service, market_account_id)
                    .context("in start_liquidity_order_book_saving")?;
            }
        }
    }

    Ok(())
}

fn save_transaction(
    ctx: &EngineContext,
    order: &OrderSnapshot,
    status: TransactionStatus,
    strategy_name: String,
) -> Result<()> {
    let mut transaction = TransactionSnapshot::new(
        order.market_id(),
        order.side(),
        order.props.raw_price,
        order.amount(),
        status,
        strategy_name,
    );

    let exchange_order_id = order
        .props
        .exchange_order_id
        .as_ref()
        .expect("`exchange_order_id` must be set before saving transaction")
        .clone();

    let fill = order
        .fills
        .fills
        .last()
        .expect("must be existed at least 1 fill on saving transaction");

    transaction.trades.push(TransactionTrade {
        exchange_order_id,
        exchange_id: order.header.exchange_account_id.exchange_id,
        price: Some(fill.price()),
        amount: fill.amount(),
        side: fill.side(),
    });

    transaction_service::save(&mut transaction, status, &ctx.event_recorder)
}

fn save_liquidity_order_book_if_can(
    ctx: &EngineContext,
    snapshots_service: &mut LocalSnapshotsService,
    market_account_id: Option<MarketAccountId>,
) -> Result<()> {
    if let Some(market_account_id) = market_account_id {
        let market_id = market_account_id.market_id();
        if let Some(snapshot) = snapshots_service.get_snapshot(market_id) {
            let exchange_account_id = market_account_id.exchange_account_id;
            let liquidity_order_book = create_liquidity_order_book_snapshot(
                snapshot,
                market_id,
                &ctx.exchanges.get(&exchange_account_id)
                    .with_expect(|| format!("exchange {exchange_account_id} should exists in `Save order book` events loop"))
                    .orders,
            );
            ctx.event_recorder
                .save(liquidity_order_book)
                .context("failed saving liquidity_order_book")?;
        }
    }

    Ok(())
}
