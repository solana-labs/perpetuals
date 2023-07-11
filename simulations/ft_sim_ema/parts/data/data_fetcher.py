
import yfinance as yf
import pandas as pd
import pytz


def fetch_data():
    tickers = ['BTC-USD', 'ETH-USD', 'SOL-USD', 'USDC-USD', 'USDT-USD']
    for ticker in tickers:
        df = yf.Ticker(ticker).history(start="2022-01-01", end="2022-12-31")

        df.reset_index(inplace=True)
        df["Date"] = [str(val)[:-15] for val in df['Date']]
        df.drop(columns=['Dividends', 'Stock Splits'], inplace=True)

        df.to_excel(f"{ticker.replace('-USD', '22')}.xlsx", index=False)


def fetch_data_hrly():
    tickers = ['BTC-USD', 'ETH-USD', 'SOL-USD', 'USDC-USD', 'USDT-USD']
    for ticker in tickers:
        df = yf.Ticker(ticker).history(period="10d", interval="1h")

        df.reset_index(inplace=True)
        df["Datetime"] = [str(val)[:-15] for val in df['Datetime']]
        df.drop(columns=['Dividends', 'Stock Splits'], inplace=True)

        df.to_excel(f"{ticker.replace('-USD', 'HR')}.xlsx", index=False)

def add_ema():
    tickers = ['BTC-USD', 'ETH-USD', 'SOL-USD', 'USDC-USD', 'USDT-USD']
    for ticker in tickers:
        df = pd.read_excel(f"{ticker.replace('-USD', 'HR')}.xlsx")
        df['ema'] = df['Close'].ewm(span=7, adjust=False).mean()
        df.to_excel(f"{ticker.replace('-USD', 'HR')}.xlsx", index=False)  

if __name__ == '__main__':
    fetch_data_hrly()
    add_ema()