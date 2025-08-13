// instrument.rs (or tests/instrument_tests.rs)
use order_book_engine::instrument::*;

#[test]
fn asset_display_and_parse() {
    assert_eq!(Asset::BTC.to_string(), "BTC");
    assert_eq!(Asset::ETH.to_string(), "ETH");
    assert_eq!(Asset::USD.to_string(), "USD");

    assert_eq!("BTC".parse::<Asset>().unwrap(), Asset::BTC);
    assert_eq!("ETH".parse::<Asset>().unwrap(), Asset::ETH);
    assert_eq!("USD".parse::<Asset>().unwrap(), Asset::USD);
    assert!("DOGE".parse::<Asset>().is_err());
}

#[test]
fn pair_display_and_parse_supported() {
    assert_eq!(BTC_USD.to_string(), "BTC-USD");
    assert_eq!(ETH_USD.to_string(), "ETH-USD");
    assert_eq!(BTC_USD.code(), "BTC-USD");
    assert_eq!(ETH_USD.code(), "ETH-USD");

    assert_eq!("BTC-USD".parse::<Pair>().unwrap(), BTC_USD);
    assert_eq!("ETH-USD".parse::<Pair>().unwrap(), ETH_USD);
}

#[test]
fn pair_parse_rejects_unsupported() {
    let e = "BTC-EUR".parse::<Pair>().unwrap_err();
    assert!(e.contains("unsupported"));
}

#[test]
fn serde_pair_is_string_roundtrip() {
    // Serialize as a plain JSON string
    let s = serde_json::to_string(&BTC_USD).unwrap();
    assert_eq!(s, "\"BTC-USD\"");

    // Deserialize back from a string
    let p: Pair = serde_json::from_str("\"ETH-USD\"").unwrap();
    assert_eq!(p, ETH_USD);
}

#[test]
fn serde_pair_rejects_object_form() {
    // Because Pair uses #[serde(try_from = "String", into = "String")],
    // an object is invalid input.
    let bad = r#"{ "base": "BTC", "quote": "USD" }"#;
    let err = serde_json::from_str::<Pair>(bad).unwrap_err().to_string();
    // error message can vary; just assert it's an error
    assert!(!err.is_empty());
}

#[test]
fn supported_and_fromstr_in_sync() {
    // Every supported pair should parse from its code and round-trip Display
    for p in Pair::supported() {
        let parsed = p.code().parse::<Pair>().unwrap();
        assert_eq!(&parsed, p);
        assert_eq!(parsed.to_string(), p.code());
    }
}

#[test]
fn crypto_usd_factory_sets_usd_quote() {
    let p = Pair::crypto_usd(Asset::BTC);
    assert_eq!(p, BTC_USD);

    let p2 = Pair::crypto_usd(Asset::ETH);
    assert_eq!(p2, ETH_USD);
}

#[test]
fn pair_is_hashable_and_equatable() {
    use std::collections::HashMap;
    let mut m = HashMap::new();
    m.insert(BTC_USD.clone(), 42u32);
    assert_eq!(m.get(&"BTC-USD".parse::<Pair>().unwrap()), Some(&42));
}

#[test]
fn asset_serde_as_string() {
    let s = serde_json::to_string(&Asset::BTC).unwrap();
    assert_eq!(s, "\"BTC\"");
    let a: Asset = serde_json::from_str("\"ETH\"").unwrap();
    assert_eq!(a, Asset::ETH);
}
