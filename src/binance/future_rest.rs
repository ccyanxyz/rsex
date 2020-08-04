use crate::binance::types::*;
use crate::errors::*;
use crate::models::*;
use crate::traits::*;
use crate::utils::*;

use hex::encode as hex_encode;
use reqwest::blocking::Response;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, USER_AGENT};
use reqwest::StatusCode;
use ring::{digest, hmac};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
pub struct BinanceSwap {
    api_key: String,
    secret_key: String,
    host: String, // https://fapi.binance.com
}

impl BinanceSwap {
    pub fn new(api_key: Option<String>, secret_key: Option<String>, host: String) -> Self {
        BinanceSwap {
            api_key: api_key.unwrap_or_else(|| "".into()),
            secret_key: secret_key.unwrap_or_else(|| "".into()),
            host,
        }
    }

    pub fn get(&self, endpoint: &str, request: &str) -> APIResult<String> {
        let mut url: String = format!("{}{}", self.host, endpoint);
        if !request.is_empty() {
            url.push_str(format!("?{}", request).as_str());
        }
        let response = reqwest::blocking::get(url.as_str())?;
        self.handler(response)
    }

    pub fn post(&self, endpoint: &str) -> APIResult<String> {
        let url: String = format!("{}{}", self.host, endpoint);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url.as_str())
            .headers(self.build_headers(false)?)
            .send()?;

        self.handler(resp)
    }

    pub fn put(&self, endpoint: &str, key: &str) -> APIResult<String> {
        let url: String = format!("{}{}", self.host, endpoint);
        let data: String = format!("listenKey={}", key);

        let client = reqwest::blocking::Client::new();
        let resp = client
            .put(url.as_str())
            .headers(self.build_headers(false)?)
            .body(data)
            .send()?;
        self.handler(resp)
    }

    pub fn delete(&self, endpoint: &str, key: &str) -> APIResult<String> {
        let url: String = format!("{}{}", self.host, endpoint);
        let data: String = format!("listenKey={}", key);

        let client = reqwest::blocking::Client::new();
        let resp = client
            .delete(url.as_str())
            .headers(self.build_headers(false)?)
            .body(data)
            .send()?;
        self.handler(resp)
    }

    pub fn get_signed(&self, endpoint: &str, request: &str) -> APIResult<String> {
        let url = self.sign(endpoint, request);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .get(url.as_str())
            .headers(self.build_headers(true)?)
            .send()?;
        self.handler(resp)
    }

    pub fn post_signed(&self, endpoint: &str, request: &str) -> APIResult<String> {
        let url = self.sign(endpoint, request);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url.as_str())
            .headers(self.build_headers(true)?)
            .send()?;
        self.handler(resp)
    }

    pub fn delete_signed(&self, endpoint: &str, request: &str) -> APIResult<String> {
        let url = self.sign(endpoint, request);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .delete(url.as_str())
            .headers(self.build_headers(true)?)
            .send()?;
        self.handler(resp)
    }

    fn sign(&self, endpoint: &str, request: &str) -> String {
        let key = hmac::SigningKey::new(&digest::SHA256, self.secret_key.as_bytes());
        let signature = hex_encode(hmac::sign(&key, request.as_bytes()).as_ref());
        let body: String = format!("{}&signature={}", request, signature);
        let url: String = format!("{}{}?{}", self.host, endpoint, body);
        url
    }

    fn build_signed_request(&self, mut params: BTreeMap<String, String>) -> APIResult<String> {
        params.insert("recvWindow".into(), "5000".to_string());

        if let Ok(ts) = get_timestamp() {
            params.insert("timestamp".into(), ts.to_string());
            let mut req = String::new();
            for (k, v) in &params {
                let param = format!("{}={}&", k, v);
                req.push_str(param.as_ref());
            }
            req.pop();
            Ok(req)
        } else {
            Err(Box::new(ExError::ApiError("get_timestamp failed".into())))
        }
    }

    fn build_headers(&self, content_type: bool) -> APIResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("rsquant"));
        if content_type {
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            );
        }
        headers.insert(
            HeaderName::from_static("x-mbx-apikey"),
            HeaderValue::from_str(self.api_key.as_str())?,
        );
        Ok(headers)
    }

    fn handler(&self, resp: Response) -> APIResult<String> {
        match resp.status() {
            StatusCode::OK => {
                let body = resp.text()?;
                Ok(body)
            }
            s => Err(Box::new(ExError::ApiError(format!("response: {:?}", s)))),
        }
    }

    pub fn get_symbols(&self) -> APIResult<Vec<SymbolInfo>> {
        let uri = "/fapi/v1/exchangeInfo";
        let ret = self.get(uri, "")?;
        let resp: ExchangeInfo = serde_json::from_str(&ret)?;
        let symbols = resp
            .symbols
            .into_iter()
            .map(|symbol| symbol.into())
            .collect::<Vec<SymbolInfo>>();
        Ok(symbols)
    }
}

impl FutureRest for BinanceSwap {
    fn get_orderbook(&self, symbol: &str, depth: u8) -> APIResult<Orderbook> {
        let uri = "/fapi/v1/depth";
        let params = format!("symbol={}&limit={}", symbol, depth);
        let ret = self.get(uri, &params)?;
        let resp: RawOrderbook = serde_json::from_str(&ret)?;
        Ok(resp.into())
    }

    fn get_ticker(&self, symbol: &str) -> APIResult<Ticker> {
        let uri = "/fapi/v1/ticker/bookTicker";
        let params = format!("symbol={}", symbol);
        let ret = self.get(uri, &params)?;
        let resp: RawTicker = serde_json::from_str(&ret)?;

        Ok(resp.into())
    }

    fn get_kline(&self, symbol: &str, period: &str, limit: u16) -> APIResult<Vec<Kline>> {
        let uri = "/fapi/v1/klines";
        let params = format!("symbol={}&interval={}&limit={}", symbol, period, limit);
        let ret = self.get(uri, &params)?;
        let resp: Vec<Vec<Value>> = serde_json::from_str(&ret)?;
        let klines = resp
            .iter()
            .map(|kline| Kline {
                timestamp: to_i64(&kline[0]) as u64,
                open: to_f64(&kline[1]),
                high: to_f64(&kline[2]),
                low: to_f64(&kline[3]),
                close: to_f64(&kline[4]),
                volume: to_f64(&kline[5]),
            })
            .collect::<Vec<Kline>>();

        Ok(klines)
    }

    fn get_balance(&self, asset: &str) -> APIResult<Balance> {
        let uri = "/fapi/v2/account";
        let params: BTreeMap<String, String> = BTreeMap::new();
        let req = self.build_signed_request(params)?;
        let ret = self.get_signed(uri, &req)?;
        let val: RawSwapAccount = serde_json::from_str(&ret)?;
        let balance = val.assets
            .iter()
            .find(|balance| balance.asset == asset);
        match balance {
            Some(bal) => {
                Ok(Balance {
                    asset: asset.into(),
                    free: str_to_f64(&bal.available_balance),
                    locked: str_to_f64(&bal.wallet_balance) - str_to_f64(&bal.available_balance),
                })
            },
            None => {
                Err(Box::new(ExError::ApiError("asset not found".into())))
            }
        }
    }

    fn create_order(
        &self,
        symbol: &str,
        price: f64,
        amount: f64,
        action: &str,
        order_type: &str,
    ) -> APIResult<String> {
        let uri = "/fapi/v1/order";
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        params.insert("symbol".into(), symbol.into());
        params.insert("side".into(), action.into());
        params.insert("type".into(), order_type.into());
        params.insert("timeInForce".into(), "GTC".into());
        params.insert("quantity".into(), amount.to_string());
        params.insert("price".into(), price.to_string());
        let req = self.build_signed_request(params)?;
        let ret = self.post_signed(uri, &req)?;
        let resp: OrderResult = serde_json::from_str(&ret)?;

        Ok(resp.order_id.to_string())
    }

    fn cancel(&self, id: &str) -> APIResult<bool> {
        let uri = "/fapi/v1/order";
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        params.insert("orderId".into(), id.into());
        let req = self.build_signed_request(params)?;
        let _ret = self.delete_signed(uri, &req)?;
        Ok(true)
    }

    fn cancel_all(&self, symbol: &str) -> APIResult<bool> {
        let uri = "/fapi/v1/allOpenOrders";
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        params.insert("symbol".into(), symbol.into());
        let req = self.build_signed_request(params)?;
        let _ret = self.delete_signed(uri, &req)?;
        Ok(true)
    }

    fn get_order(&self, id: &str) -> APIResult<Order> {
        let uri = "/fapi/v1/order";
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        params.insert("orderId".into(), id.into());
        let req = self.build_signed_request(params)?;
        let ret = self.get_signed(uri, &req)?;
        let resp: RawOrder = serde_json::from_str(&ret)?;

        Ok(resp.into())
    }

    fn get_open_orders(&self, symbol: &str) -> APIResult<Vec<Order>> {
        let uri = "/fapi/v1/openOrder";
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        params.insert("symbol".into(), symbol.into());
        let req = self.build_signed_request(params)?;
        let ret = self.get_signed(uri, &req)?;
        let resp: Vec<RawOrder> = serde_json::from_str(&ret)?;

        let orders = resp
            .into_iter()
            .map(|order| order.into())
            .collect::<Vec<Order>>();
        Ok(orders)
    }

    fn get_history_orders(&self, symbol: &str) -> APIResult<Vec<Order>> {
        unimplemented!()
    }
}

#[cfg(test)]
mod test {
    #![allow(dead_code)]
    use super::*;

    const API_KEY: &'static str =
        "N9QAtGjFuNXDAnvMlidLzfvGargt54mKQuQbzyafO2hg5Hr8YNHV1e2Jfavi44nK";
    const SECRET_KEY: &'static str =
        "lCuul7mVApKczbGJBrAgqEIWTWwbQ1BTMBPJyvK19q2BNmlsd5718cAWWByNuY5N";
    const HOST: &'static str = "https://api.binance.com";

    #[test]
    fn test_get_orderbook() {
        let api = BinanceSwap::new(None, None, "https://www.binancezh.com".to_string());
        let ret = api.get_orderbook("BTCUSDT", 10);
        println!("{:?}", ret);
    }

    #[test]
    fn test_get_ticker() {
        let api = BinanceSwap::new(None, None, "https://www.binancezh.com".to_string());
        let ret = api.get_ticker("BTCUSDT");
        println!("{:?}", ret);
    }

    #[test]
    fn test_get_kline() {
        let api = BinanceSwap::new(None, None, "https://www.binancezh.com".to_string());
        let ret = api.get_kline("BTCUSDT", "1m", 10);
        println!("{:?}", ret);
        println!("{:?}", ret.unwrap().len());
    }

    #[test]
    fn test_get_balance() {
        let api = BinanceSwap::new(Some(API_KEY.into()), Some(SECRET_KEY.into()), HOST.into());
        let ret = api.get_balance("USDT");
        println!("{:?}", ret);
    }

    //#[test]
    fn test_create_order() {
        let api = BinanceSwap::new(Some(API_KEY.into()), Some(SECRET_KEY.into()), HOST.into());
        let ret = api.create_order("BTCUSDT".into(), 9000.0, 0.01, "BUY", "LIMIT");
        println!("{:?}", ret);
    }
}
