
use axum::{routing::get, Router, extract::Query, response::Json};
use std::net::SocketAddr;
use chrono::{Utc, NaiveDateTime, TimeZone};
use tracing_subscriber::FmtSubscriber;
use ethers::{abi::Abi, contract::Contract, core::k256::elliptic_curve::rand_core::block, providers:: { Http, Middleware, Provider}, types::Address};
use ethers::core::types::U256;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use serde_json::{self, Value};
use std::marker::Send;
use ethers::types::{Filter, Log, H160, H256, U64, I256, Block, BlockNumber};
use ethers::abi::RawLog;
use ethers::contract::EthLogDecode;
use ethers::contract::EthEvent;
use ethers::utils::hex;

use std::str::FromStr;

#[derive(Deserialize)]
struct PoolDataQuery {
    token_a: String,
    token_b: String,
    start_timestamp: String,
    end_timestamp: String,
    interval: Option<String>,
}

#[derive(Debug, EthEvent, Serialize)]
#[ethevent(name = "Swap", abi = "Swap(address indexed sender, address indexed to, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)")]
struct SwapEvent {
    sender: Address,
    to: Address,
    amount0: I256,
    amount1: I256,
    sqrt_price_x96: U256,
    liquidity: U256,
    tick: i32,  // ABI's int24 can fit in i32 in Rust
}

#[derive(Debug, EthEvent, Serialize)]
#[ethevent(name = "Mint", abi = "Mint(address sender, address indexed owner, int24 indexed tickLower, int24 indexed tickUpper, uint128 amount, uint256 amount0, uint256 amount1)")]
struct MintEvent {
    sender: Address,
    owner: Address,
    tick_lower: i32,  // int24 fits in i32
    tick_upper: i32,  // int24 fits in i32
    amount: U256,
    amount0: U256,
    amount1: U256,
}

#[derive(Debug, EthEvent, Serialize)]
#[ethevent(name = "Burn", abi = "Burn(address indexed owner, int24 indexed tickLower, int24 indexed tickUpper, uint128 amount, uint256 amount0, uint256 amount1)")]
struct BurnEvent {
    owner: Address,
    tick_lower: i32,  // int24 fits in i32
    tick_upper: i32,  // int24 fits in i32
    amount: U256,
    amount0: U256,
    amount1: U256,
}

#[derive(Debug, EthEvent, Serialize)]
#[ethevent(name = "Collect", abi = "Collect(address indexed owner, address recipient, int24 indexed tickLower, int24 indexed tickUpper, uint128 amount0, uint128 amount1)")]
struct CollectEvent {
    owner: Address,
    recipient: Address,
    tick_lower: i32,  // int24 fits in i32
    tick_upper: i32,  // int24 fits in i32
    amount0: U256,
    amount1: U256,
}

#[derive(Debug, Serialize)]
enum UniswapEvent {
    Swap(SwapEvent),
    Mint(MintEvent),
    Burn(BurnEvent),
    Collect(CollectEvent),
}

impl EthLogDecode for UniswapEvent {
    fn decode_log(log: &RawLog) -> Result<Self, ethers::abi::Error> {
        decode_uniswap_event(&Log {
            address: H160::zero(),
            topics: log.topics.clone(),
            data: log.data.clone().into(),
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            transaction_log_index: None,
            log_type: None,
            removed: None,
        }).map_err(|e| ethers::abi::Error::InvalidData)
    }
}
fn decode_uniswap_event(log: &Log) -> Result<UniswapEvent, Box<dyn std::error::Error + Send + Sync>> {
    // Event signatures for Uniswap V3 pool events
    let swap_signature = H256::from_slice(&hex::decode("c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67").unwrap());
    let mint_signature = H256::from_slice(&hex::decode("7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde").unwrap());
    let burn_signature = H256::from_slice(&hex::decode("0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c").unwrap());
    let collect_signature = H256::from_slice(&hex::decode("70935338e69775456a85ddef226c395fb668b63fa0115f5f20610b388e6ca9c0").unwrap());

    // Parse the raw log data
    let raw_log = RawLog {
        topics: log.topics.clone(),
        data: log.data.to_vec(),
    };

    // Match based on event signature and decode the appropriate event
    if log.topics[0] == swap_signature {
        match <SwapEvent as EthLogDecode>::decode_log(&raw_log) {
            Ok(event) => return Ok(UniswapEvent::Swap(event)),
            Err(err) => return Err(Box::new(err)),
        }
    } else if log.topics[0] == mint_signature {
        match <MintEvent as EthLogDecode>::decode_log(&raw_log) {
            Ok(event) => return Ok(UniswapEvent::Mint(event)),
            Err(err) => return Err(Box::new(err)),
        }
    } else if log.topics[0] == burn_signature {
        match <BurnEvent as EthLogDecode>::decode_log(&raw_log) {
            Ok(event) => return Ok(UniswapEvent::Burn(event)),
            Err(err) => return Err(Box::new(err)),
        }
    } else if log.topics[0] == collect_signature {
        match <CollectEvent as EthLogDecode>::decode_log(&raw_log) {
            Ok(event) => return Ok(UniswapEvent::Collect(event)),
            Err(err) => return Err(Box::new(err)),
        }
    } else {
        println!("Unknown event signature: {:?}", log);
    }
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Unknown event signature")))
}

async fn get_pool_address(provider: Arc<Provider<Http>>, factory_address: Address, token0: Address, token1: Address, fee: u32) -> Result<Address, Box<dyn std::error::Error + Send + Sync>> {
    // Load the Uniswap V3 factory ABI
    let abi_file = File::open("src/contracts/uniswap_pool_factory_abi.json")?;
    let abi: Abi = serde_json::from_reader(BufReader::new(abi_file))?;

    // Instantiate the contract
    let factory = Contract::new(factory_address, abi, provider.clone());

    // Call the getPool function
    let pool_address: Address = factory.method("getPool", (token0, token1, U256::from(fee)))?.call().await?;

    Ok(pool_address)
}


async fn get_pool_events(
    provider: Arc<Provider<Http>>,
    pool_address: H160,
    from_block: U64,
    to_block: U64
) -> Result<Vec<Log>, Box<dyn std::error::Error + Send + Sync>> {
    let filter = Filter::new()
        .address(pool_address)
        .from_block(from_block)
        .to_block(to_block);
    println!("from_block: {:?}, to_block: {:?}", from_block, to_block);
    let logs = provider.get_logs(&filter).await?;
    
    Ok(logs)
}

fn use_tracing_subscriber() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    use_tracing_subscriber();
    env_logger::init();

    // Define the route for fetching pool data
    let app = Router::new().route("/pool-data", get(get_pool_data));

    // Set up the server address
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Server running at http://{}", addr);

    // Start the Axum server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    // let ws = Ws::connect("ws://localhost:8546").await?;
    // let provider = Arc::new(Provider::new(ws));

    Ok(())
}

/// Retrieves pool data for a given pair of tokens and a time range.
///
/// # Query Parameters
///
/// - `token_a` (string, required): The address of the first token.
/// - `token_b` (string, required): The address of the second token.
/// - `start_timestamp` (integer, required): The start timestamp in Unix format.
/// - `end_timestamp` (integer, required): The end timestamp in Unix format.
/// - `interval` (string, optional): The interval for data aggregation (default: "1h").
///
/// # Example Request
///
/// ```sh
/// curl -X GET "http://localhost:8080/pool-data?token_a=0xTokenAAddress&token_b=0xTokenBAddress&start_timestamp=1638316800&end_timestamp=1638403200&interval=1h"
/// ```
///
/// # Success Response
///
/// - **Code**: `200 OK`
/// - **Content**:
///
/// ```json
/// {
///     "token_a": "0xTokenAAddress",
///     "token_b": "0xTokenBAddress",
///     "start_timestamp": 1638316800,
///     "end_timestamp": 1638403200,
///     "interval": "1h",
///     "data": "some pool data"
/// }
/// ```
///
/// # Error Responses
///
/// - **Code**: `400 Bad Request`
/// - **Content**:
///
/// ```json
/// {
///     "error": "Invalid input: Token addresses cannot be empty"
/// }
/// ```
///
/// - **Code**: `500 Internal Server Error`
/// - **Content**:
///
/// ```json
/// {
///     "error": "Unknown error"
/// }
/// ```
async fn get_pool_data(Query(params): Query<PoolDataQuery>) -> Json<Value> {
    // Extract the query parameters
    let token_a = &params.token_a;
    let token_b = &params.token_b;
    let start_timestamp = &params.start_timestamp;
    let end_timestamp = &params.end_timestamp;
    let interval = params.interval.clone().unwrap_or("1h".to_string()); // Default interval to 1 hour if not provided

    // Fetch the pool data
    match fetch_pool_data(token_a, token_b, start_timestamp, end_timestamp, &interval).await {
        Ok(data) => Json(data),    // Return JSON response on success
        Err(err) => Json(serde_json::json!({ "error": format!("Error fetching data: {}", err) })),
    }
}

/// Fetches pool data for a given pair of tokens and a time range.
///
/// # Arguments
///
/// - `token_a`: The address of the first token.
/// - `token_b`: The address of the second token.
/// - `start_timestamp`: The start timestamp in Unix format.
/// - `end_timestamp`: The end timestamp in Unix format.
/// - `interval`: The interval for data aggregation.
///
/// # Returns
///
/// A JSON value containing the pool data.
///
/// # Errors
///
/// Returns an error if the data fetching fails.
async fn fetch_pool_data(token_a: &str, token_b: &str, start_datetime: &str, end_datetime: &str, interval: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    
    // Connect to the Ethereum provider
    // let ws = Ws::connect("ws://localhost:8546").await?;
    // let provider = Arc::new(Provider::new(ws));
    // let rpc_url = "https://eth.llamarpc.com";
    let rpc_url = "https://geth.hyperclouds.io";
    let provider: Arc<Provider<Http>> = Arc::new(Provider::<Http>::try_from(rpc_url)?);

    // Get the Uniswap V3 factory address
    let factory_address = Address::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")?;

    // Get the pool address for the given token pair
    let token_a_address = Address::from_str(token_a)?;
    let token_b_address = Address::from_str(token_b)?;
    let pool_address = get_pool_address(provider.clone(), factory_address, token_a_address, token_b_address, 3000).await?;
    println!("Fetched pool address: {:?}", pool_address);

    // let date_str = "2024-09-27 19:34:56";
    let first_naive_datetime = NaiveDateTime::parse_from_str(start_datetime, "%Y-%m-%d %H:%M:%S")
        .expect("Failed to parse date");
    let first_datetime_utc = Utc.from_utc_datetime(&first_naive_datetime);
    let first_timestamp = first_datetime_utc.timestamp() as u64;

    let second_naive_datetime = NaiveDateTime::parse_from_str(end_datetime, "%Y-%m-%d %H:%M:%S")
        .expect("Failed to parse date");
    let second_datetime_utc = Utc.from_utc_datetime(&second_naive_datetime);
    let second_timestamp = second_datetime_utc.timestamp() as u64;

    // Check if the given date time is more than the current date time
    let current_timestamp = Utc::now().timestamp() as u64;
    if first_timestamp > current_timestamp {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Given date time is in the future")));
    }

    // let block_number = provider.get_block_number().await?;
    let average_block_time = get_average_block_time(provider.clone()).await?;

    let block_number_by_first_timestamp = get_block_number_from_timestamp(provider.clone(), first_timestamp, average_block_time).await?;
    let block_number_by_second_timestamp = block_number_by_first_timestamp + (second_timestamp - first_timestamp) / average_block_time;

    // Get the pool events
    let from_block = block_number_by_first_timestamp;
    let to_block = block_number_by_second_timestamp;
    let logs = get_pool_events(provider.clone(), pool_address, from_block, to_block).await?;
    println!("Fetched {} logs", logs.len());
    

    let mut data = Vec::new();
    // Decode the logs
    for log in logs {
        data.push(decode_uniswap_event(&log));
    }

    let data: Vec<_> = data.into_iter().map(|result| {
        result.map_err(|e| e.to_string())
    }).collect();
    Ok(serde_json::json!({ "data": data }))
}

const NUM_BLOCKS: u64 = 100; // Number of blocks to consider for average block time calculation

async fn get_average_block_time(provider: Arc<Provider<Http>>) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch the latest block
    let latest_block: Block<H256> = provider.get_block(BlockNumber::Latest).await?.ok_or("Latest block not found")?;
    let latest_block_number = latest_block.number.ok_or("Latest block number not found")?;

    // Create a vector of tasks to fetch block timestamps concurrently
    let mut tasks = Vec::new();
    for i in 0..NUM_BLOCKS {
        let provider = provider.clone();
        let block_number = latest_block_number - U64::from(i);
        tasks.push(tokio::spawn(async move {
            let block: Block<H256> = provider.get_block(block_number).await?.ok_or("Block not found")?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(block.timestamp.as_u64())
        }));
    }

    // Collect the results
    let mut timestamps = Vec::new();
    for task in tasks {
        timestamps.push(task.await??);
    }

    // Calculate the time differences between consecutive blocks
    let mut time_diffs = Vec::new();
    for i in 1..timestamps.len() {
        time_diffs.push(timestamps[i - 1] - timestamps[i]);
    }

    // Compute the average block time
    let total_time_diff: u64 = time_diffs.iter().sum();
    let average_block_time = total_time_diff / time_diffs.len() as u64;

    Ok(average_block_time)
}

async fn get_block_number_from_timestamp(
    provider: Arc<Provider<Http>>,
    timestamp: u64,
    average_block_time: u64
) -> Result<U64, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch the latest block
    let latest_block: Block<H256> = provider.get_block(BlockNumber::Latest).await?.ok_or("Latest block not found")?;
    let latest_block_number = latest_block.number.ok_or("Latest block number not found")?;
    let latest_block_timestamp = latest_block.timestamp.as_u64();

    // Estimate the block number using the average block time
    let estimated_block_number = latest_block_number.as_u64() - (latest_block_timestamp - timestamp) / average_block_time;

    // Perform exponential search to find the range
    let mut low = U64::zero();
    let mut high = latest_block_number;
    let mut mid = U64::from(estimated_block_number);

    while low < high {
        let block: Block<H256> = provider.get_block(mid).await?.ok_or("Block not found")?;
        let block_timestamp = block.timestamp.as_u64();

        if block_timestamp < timestamp {
            low = mid + 1;
        } else {
            high = mid;
        }

        // Adjust mid for exponential search
        mid = (low + high) / 2;
    }

    Ok(low)
}