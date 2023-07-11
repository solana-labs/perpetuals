
import yfinance as yf
import pandas as pd
import pytz
import os

def fetch_data():
    tickers = ['BTC-USD', 'ETH-USD', 'SOL-USD', 'USDC-USD', 'USDT-USD']
    for ticker in tickers:
        df = yf.Ticker(ticker).history(start="2023-01-02", end="2023-01-24", interval="1h")

        df.reset_index(inplace=True)
        df["Datetime"] = [str(val)[:-15] for val in df['Datetime']]
        df.drop(columns=['Dividends', 'Stock Splits'], inplace=True)

        df.to_csv(f"{ticker.replace('-USD', '-janvol')}.csv", index=False)

def add_ema():
    tickers = ['BTC-USD', 'ETH-USD', 'SOL-USD', 'USDC-USD', 'USDT-USD']
    for ticker in tickers:
        df = pd.read_csv(f"{ticker.replace('-USD', '-janvol')}.csv")
        df['ema'] = df['Close'].ewm(span=7, adjust=False).mean()
        df.to_csv(f"{ticker.replace('-USD', '-janvol')}.csv", index=False)  


def split_files_by_day():
    # Get a list of files in the directory
    directory = os.getcwd()
    files = os.listdir(directory)

    # Iterate through each file
    for file in files:
        if file.endswith(".py"):
            continue

        file_path = os.path.join(directory, file)

        # Read the original file into a DataFrame
        df = pd.read_csv(file_path)

        # Convert the 'Date' column to datetime if needed
        df['Date'] = pd.to_datetime(df['Datetime'])

        # Group the data by date and iterate through each day
        for date, data in df.groupby(df['Date'].dt.date):
            # Create a new DataFrame for each day's data
            new_df = pd.DataFrame(data)

            # Create a new file name based on the date
            new_file_name = f"{file.replace('.csv', f'-{date}')}.csv"
            new_file_path = os.path.join(directory, new_file_name)

            # Save the new DataFrame as a separate file for each day
            new_df.to_csv(new_file_path, index=False)

        # Remove the original file
        os.remove(file_path)


if __name__ == '__main__':
    fetch_data()
    add_ema()
    split_files_by_day()
