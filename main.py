import os
import math
import json
import time
import argparse
from datetime import datetime
from py_clob_client.client import ClobClient
from py_clob_client.clob_types import BookParams

def get_open_markets(host: str, api_key: str, chain_id: int = 137):
    client = ClobClient(host, key=api_key, chain_id=chain_id)
    open_markets = []
    next_cursor = ""
    while True:
        if next_cursor and next_cursor != 'LTE=':
            response = client.get_markets(next_cursor=next_cursor)
        else:
            response = client.get_markets()
        for market in response['data']:
            if market.get('active', False) and not market.get('closed', True):
                open_markets.append(market)
        next_cursor = response.get('next_cursor')
        if not next_cursor or next_cursor == 'LTE=':
            break
    return open_markets

def get_order_book(client, token_id):
    # token_id is required by the API to fetch the order book
    return client.get_order_book(token_id=token_id)

def main(log_file=None):
    # Configure endpoint and API key via environment variables
    host = os.environ.get("CLOB_ENDPOINT", "https://clob.polymarket.com")
    api_key = os.environ.get("POLY_API_KEY", "")
    client = ClobClient(host, key=api_key)
    markets = get_open_markets(host, api_key)
    token_id_to_market = {}
    token_ids = []
    for m in markets:
        tokens = m.get('tokens')
        if tokens and isinstance(tokens, list):
            for token in tokens:
                token_id = token.get('token_id')
                if token_id:
                    token_ids.append(token_id)
                    token_id_to_market[token_id] = m
    if token_ids:
        # Batch fetch order books in chunks to avoid 413 error
        chunk_size = 50
        request_date = datetime.utcnow().isoformat()
        lines = []
        batch_start = time.time()
        for i in range(0, len(token_ids), chunk_size):
            chunk = token_ids[i:i+chunk_size]
            params = [BookParams(token_id=tid) for tid in chunk]
            order_books = client.get_order_books(params=params)
            for ob in order_books:
                token_id = getattr(ob, 'asset_id', None)
                market = token_id_to_market.get(token_id, {})
                market_slug = market.get('market_slug', 'unknown')
                # Prepare JSON line
                line = json.dumps({
                    "request_date": request_date,
                    "market_slug": market_slug,
                    "token_id": token_id,
                    "order_book": ob.__dict__
                })
                lines.append(line)
            # Rate limit: no more than 50 requests every 10 seconds
            if (i // chunk_size + 1) % 50 == 0:
                elapsed = time.time() - batch_start
                if elapsed < 10:
                    time.sleep(10 - elapsed)
                batch_start = time.time()
        if log_file:
            with open(log_file, 'a') as f:
                for line in lines:
                    f.write(line + '\n')
        else:
            for line in lines:
                print(line)
    else:
        print("No token_ids found for any open market.")

def run_every_x_minutes(interval, log_file=None):
    while True:
        main(log_file=log_file)
        time.sleep(interval * 60)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Polymarket order book fetcher")
    parser.add_argument('--time', type=int, default=5, help='Interval in minutes between fetches')
    parser.add_argument('--log', type=str, default=None, help='File to append JSON lines output to')
    args = parser.parse_args()
    run_every_x_minutes(args.time, log_file=args.log)