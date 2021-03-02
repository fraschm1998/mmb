use chrono::Utc;
use mmb_lib::core as mmb;
use mmb_lib::core::exchanges::binance::*;
use mmb_lib::core::exchanges::cancellation_token::CancellationToken;
use mmb_lib::core::exchanges::common::*;
use mmb_lib::core::exchanges::exchange::*;
use mmb_lib::core::exchanges::exchange_features::*;
use mmb_lib::core::orders::order::*;
use mmb_lib::core::settings;
use rust_decimal_macros::*;
use std::env;
use std::thread;
use std::time::Duration;

#[actix_rt::test]
async fn open_orders_exists() {
    // Get data to access binance account
    let api_key = env::var("BINANCE_API_KEY");
    if api_key.is_err() {
        dbg!("Environment variable BINANCE_API_KEY are not set. Unable to continue test");
        return;
    }

    let secret_key = env::var("BINANCE_SECRET_KEY");
    if secret_key.is_err() {
        dbg!("Environment variable BINANCE_SECRET_KEY are not set. Unable to continue test");
        return;
    }

    let settings = settings::ExchangeSettings {
        api_key: api_key.unwrap(),
        secret_key: secret_key.unwrap(),
        is_marging_trading: false,
        web_socket_host: "".into(),
        web_socket2_host: "".into(),
        rest_host: "https://api.binance.com".into(),
    };

    let exchange_account_id: ExchangeAccountId = "Binance0".parse().unwrap();
    let binance = Binance::new(settings, exchange_account_id.clone());

    let websocket_host = "wss://stream.binance.com:9443".into();
    let currency_pairs = vec!["PHBBTC".into()];
    let channels = vec!["depth".into(), "trade".into()];

    let exchange = Exchange::new(
        exchange_account_id,
        websocket_host,
        currency_pairs,
        channels,
        Box::new(binance),
        ExchangeFeatures::new(OpenOrdersType::AllCurrencyPair, false),
    );

    exchange.clone().connect().await;

    let test_currency_pair = CurrencyPair::from_currency_codes("phb".into(), "btc".into());
    let order_header = OrderHeader::new(
        ClientOrderId::unique_id(),
        Utc::now(),
        mmb::exchanges::common::ExchangeAccountId::new("".into(), 0),
        test_currency_pair.clone(),
        OrderType::Limit,
        OrderSide::Buy,
        dec!(10000),
        OrderExecutionType::None,
        ReservationId::gen_new(),
        None,
        "".into(),
    );

    let order_to_create = OrderCreating {
        header: order_header,
        // It has to be between (current price on exchange * 0.2) and (current price on exchange * 5)
        price: dec!(0.00000004),
    };

    exchange
        .create_order(&order_to_create, CancellationToken::default())
        .await
        .unwrap();

    let second_order_header = OrderHeader::new(
        ClientOrderId::unique_id(),
        Utc::now(),
        mmb::exchanges::common::ExchangeAccountId::new("".into(), 0),
        test_currency_pair.clone(),
        OrderType::Limit,
        OrderSide::Buy,
        dec!(10000),
        OrderExecutionType::None,
        ReservationId::gen_new(),
        None,
        "".into(),
    );

    let second_order_to_create = OrderCreating {
        header: second_order_header,
        // It has to be between (current price on exchange * 0.2) and (current price on exchange * 5)
        price: dec!(0.00000004),
    };
    exchange
        .create_order(&second_order_to_create, CancellationToken::default())
        .await
        .unwrap();

    // Binance can process new orders close to 10 seconds
    thread::sleep(Duration::from_secs(10));
    let all_orders = exchange.get_open_orders().await.unwrap();
    dbg!(&all_orders);

    assert!(!all_orders.is_empty())
}
