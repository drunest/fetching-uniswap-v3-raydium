# Pool Data API

This project provides an API to fetch pool data for a given pair of tokens and a time range on `Uniswap V3` and `Raydium`. It is built using Rust and the Axum framework.

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [API Documentation](#api-documentation)
  - [GET /pool-data](#get-pool-data)
- [Error Handling](#error-handling)

## Installation

To build and run the project, you need to have Rust and Cargo installed. Clone the repository and run the following commands:

```sh
git clone https://github.com/drunest/fetching-uniswap-v3-raydium.git
cd fetching-uniswap-v3-raydium
cargo build
cargo run
```
## Usage
Once the server is running, you can access the API endpoint at http://localhost:8080/pool-data.

## API Documentation
```sh
GET /pool-data
```
### Description
This endpoint retrieves pool data for a given pair of tokens and a time range.

### URL
```sh
GET /pool-data
```
### Query Parameters
- `token_a` (string, required): The address of the first token.
- `token_b` (string, required): The address of the second token.
- `start_timestamp` (string, required): The start timestamp in Unix format.
- `end_timestamp` (string, required): The end timestamp in Unix format.
- `interval` (string, optional): The interval for data aggregation (default: "1h").

### Example Request
#### Using curl:
```sh
curl -X GET "http://localhost:8080/pool-data?token_a=0xTokenAAddress&token_b=0xTokenBAddress&start_timestamp=2024-09-27 10:34:56&end_timestamp=2024-09-27 19:34:56&interval=1h"
```
#### Using Python requests library:
```sh
import requests

url = "http://localhost:8080/pool-data"
params = {
    "token_a": "0xTokenAAddress",
    "token_b": "0xTokenBAddress",
    "start_timestamp": "2024-09-27 10:34:56",
    "end_timestamp": "2024-09-27 19:34:56",
    "interval": "1h"
}

response = requests.get(url, params=params)
print(response.json())
```
### Success Response
- Code: 200 OK
- Content:
```sh
{
    "token_a": "0xTokenAAddress",
    "token_b": "0xTokenBAddress",
    "start_timestamp": "2024-09-27 10:34:56",
    "end_timestamp": "2024-09-27 19:34:56",
    "interval": "1h",
    "data":
    [
        {
            "Ok": {
                "Swap": {
                    "amount0": "0x19f05a85de05ae534f0",
                    "amount1": "0xffffffffffffffffffffffffffffffffffffffffffffffffbefb67af9f11870f",
                    "liquidity": "0x28c9f1a5acec28c8b15f",
                    "sender": "0x14f2b6ca0324cd2b013ad02a7d85541d215e2906",
                    "sqrt_price_x96": "0x656db5908f88175c3a3651d",
                    "tick": -73972,
                    "to": "0x00000000063e0e1e06a0fe61e16be8bdec1bea31",
                }
            }
        },
        {
            "Ok": {
                "Swap": {
                    "amount0": "0xfffffffffffffffffffffffffffffffffffffffffffffff602418adbe5923c03",
                    "amount1": "0x192ba949641294a",
                    "liquidity": "0x28c9f1a5acec28c8b15f",
                    "sender": "0xe592427a0aece92de3edee1f18e0157c05861564",
                    "sqrt_price_x96": "0x656e53111c06aa98ce3b03c",
                    "tick": -73972,
                    "to": "0x6a000f20005980200259b80c5102003040001068"
                }
            }
        },
        {
            "Ok": {
                "Swap": {
                    "amount0": "0x2c4838beeca4f936e2",
                    "amount1": "0xfffffffffffffffffffffffffffffffffffffffffffffffff911e55500000000",
                    "liquidity": "0x28c9f1a5acec28c8b15f",
                    "sender": "0x00000000009e50a7ddb7a7b0e2ee6604fd120e49",
                    "sqrt_price_x96": "0x656b9b263ff5269d81f75bb",
                    "tick": -73974,
                    "to": "0x00000000009e50a7ddb7a7b0e2ee6604fd120e49"
                }
            }
        }
    ]
}
```
### Error Responses
**Code: `400 Bad Request`**
- Content
```sh
{
    "error": "Invalid input: Token addresses cannot be empty"
}
```
- Code: **500 Internal Server Error**
- Content:
```sh
{
    "error": "Unknown error"
}
```
### Error Handling
**Invalid Input**: Ensure that both `token_a` and `token_b` are provided and are valid addresses.
**Server Errors**: If you encounter a `500 Internal Server Error`, check the server logs for more details.