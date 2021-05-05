#[derive(Clone, Debug, PartialEq)]
pub enum RequestType {
    CreateOrder,
    CancelOrder,
    GetOrderInfo,
    GetBalance,
    GetOpenOrders,
    GetMarkets,
    GetCurrencies,
    GetOrderBook,
    GetTrades,
    GetCancelStick,
    GetActivePositions,
    ClosePosition,
    GetOrderTrades,
    GetListenKey,
    UpdateListenKey,
    GetLastTrades,
    GetFundingInfo,
    GetBalanceAndPosition,
    GetLastPrints,
    GetProfileId,
    GetMyTrades,
    SetLeverage,
}
