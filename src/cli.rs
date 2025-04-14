use clap::{Parser, Subcommand, arg, builder::PossibleValuesParser};
use std::time::SystemTime;

use crate::{
    orderbook::OrderBook,
    orders::{Order, OrderType, Side},
};

/// Simple CLI to interact with the Order Book
#[derive(Parser)]
#[command(name = "Order Book CLI")]
#[command(
    author = "Your Name",
    version = "0.1",
    about = "A demo of a limit order book"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ///Add a new order to the book
    Add {
        ///BUY or SELL
        #[arg(value_parser = PossibleValuesParser::new(["buy", "sell"]))]
        side: String,

        ///LIMIT or Market
        #[arg(value_parser = PossibleValuesParser::new(["limit", "market"]))]
        order_type: String,

        /// Price (only used for limit orders)
        price: Option<u64>,

        /// Quantity (must be > 0)
        quantity: u64,
    },

    /// Match a new market order
    Match {
        /// BUY or SELL
        #[arg(value_parser = PossibleValuesParser::new(["buy", "sell"]))]
        side: String,
        ///Quantity
        quantity: u64,
    },
    /// Display the current order book
    Book,
}

fn handle_add(
    order_book: &mut OrderBook,
    side_str: String,
    order_type_str: String,
    price: Option<u64>,
    quantity: u64,
) {
    let side = match side_str.as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => unreachable!(),
    };

    let order_type = match order_type_str.as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        _ => unreachable!(),
    };
    let order = Order {
        id: rand::random::<u64>(),
        side,
        order_type,
        price: match order_type {
            OrderType::Limit => price, //Use provided price
            OrderType::Market => None, //Price not relevant for market
        },
        quantity,
        timestamp: SystemTime::now(),
    };

    match order_type {
        OrderType::Limit => {
            order_book.add_order(order.clone());
            println!("Limit order added:  {:?}", order);
        }
        OrderType::Market => {
            let trades = order_book.match_order(order);
            if trades.is_empty() {
                println!("No trades occured.");
            } else {
                println!("Trades generated from market order: ");
                for t in trades {
                    println!("{:?}", t);
                }
            }
        }
    }
}

pub fn handle_match(order_book: &mut OrderBook, side_str: String, quantity: u64) {
    let side = match side_str.as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => unreachable!(),
    };
    let order = Order {
        id: rand::random::<u64>(),
        side,
        order_type: OrderType::Market,
        price: None,
        quantity,
        timestamp: SystemTime::now(),
    };
    let trades = order_book.match_order(order);
    if trades.is_empty() {
        println!("No trades occured");
    } else {
        println!("Trades generated");
        for t in trades {
            println!("{:?}", t);
        }
    }
}

fn print_order_book(order_book: &OrderBook) {
    println!("------ Order Book ------");
    println!("Bids (higest first):");
    for (price, orders) in order_book.bids.iter().rev() {
        let total_qty: u64 = orders.iter().map(|o| o.quantity).sum();
        println!("Price: {}, Total Qty: {}", price, total_qty);
    }

    println!("Asks (Lowest first):");
    for (price, orders) in order_book.asks.iter() {
        let total_qty: u64 = orders.iter().map(|o| o.quantity).sum();
        println!("Price: {}, Total Qty: {}", price, total_qty);
    }
    println!("--------------------------");
}
pub fn run_cli() {
    let cli = Cli::parse();
    let mut order_book = OrderBook::new();
    match cli.command {
        Commands::Add {
            side,
            order_type,
            price,
            quantity,
        } => {
            handle_add(&mut order_book, side, order_type, price, quantity);
        }
        Commands::Match { side, quantity } => {
            handle_match(&mut order_book, side, quantity);
        }
        Commands::Book => {
            print_order_book(&order_book);
        }
    }
}
