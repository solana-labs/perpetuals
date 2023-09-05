import os
import pandas as pd
import math
from datetime import datetime, timedelta


def split_files(dir):
    # Get a list of files in the directory
    directory = os.path.join(os.getcwd(), str(dir))
    files = os.listdir(directory)

    # Iterate through each file
    for file in files:

        file_path = os.path.join(directory, file)

        # Read the original file into a DataFrame
        df = pd.read_csv(file_path)
        print(file_path)
        if file.startswith("U"):
            cols_to_drop = ['s', 'o', 'h', 'l', 'v']
            df = df.drop(cols_to_drop, axis=1) 
            df['time'] = pd.to_datetime(df['t'])
            df = df.drop('t', axis=1) 
        else:
            cols_to_drop = ['s', 'c', 'o', 'h', 'l', 'v', 'slot', 'publishedSlot', 'confidence', 'emaPrice', 'emaDifference%', 'spread', 'spot_confidence%', 'spread%']
            df = df.drop(cols_to_drop, axis=1) 
            # Convert the 'Date' column to datetime if needed
            df['time'] = pd.to_datetime(df['time'])

        # Group the data by date and iterate through each day
        for hour, data in df.groupby(df['time'].dt.hour):

            # Create a new DataFrame for each day's data
            new_df = pd.DataFrame(data)

            # Create a new file name based on the date
            new_file_name = f"{file.replace('.csv', f'-{hour}').upper()}.csv"
            new_file_path = os.path.join(directory, new_file_name)

            new_df['time'] = new_df['time'].dt.minute

            # Save the new DataFrame as a separate file for each day
            new_df.to_csv(new_file_path, index=False)
            print(file, len(new_df))

        # Remove the original file
        os.remove(file_path)

def fill_dir(num):
    # create a directory called 1
    directory = os.path.join(os.getcwd(), str(num))
    if not os.path.exists(directory):
        os.makedirs(directory)

    # copy file btc-1.csv from dir BTC into new dir
    file_path = os.path.join(os.getcwd(), 'Btc', f'btc-{num}.csv')
    new_file_path = os.path.join(directory, f'btc-{num}.csv')
    df = pd.read_csv(file_path)
    df.to_csv(new_file_path, index=False)

    file_path = os.path.join(os.getcwd(), 'Eth', f'eth-{num}.csv')
    new_file_path = os.path.join(directory, f'eth-{num}.csv')
    df = pd.read_csv(file_path)
    df.to_csv(new_file_path, index=False)

    file_path = os.path.join(os.getcwd(), 'Sol', f'sol-{num}.csv')
    new_file_path = os.path.join(directory, f'sol-{num}.csv')
    df = pd.read_csv(file_path)
    df.to_csv(new_file_path, index=False)

    file_path = os.path.join(os.getcwd(), 'Stables', f'USDC-{num}.csv')
    new_file_path = os.path.join(directory, f'USDC-{num}.csv')
    df = pd.read_csv(file_path)
    df.to_csv(new_file_path, index=False)

    file_path = os.path.join(os.getcwd(), 'Stables', f'USDT-{num}.csv')
    new_file_path = os.path.join(directory, f'USDT-{num}.csv')
    df = pd.read_csv(file_path)
    df.to_csv(new_file_path, index=False)


def main():
    for i in range (1, 9):
        fill_dir(i)
        print(f'{i} dir created')
        split_files(i)
        print(f'{i} splitted')


if __name__ == '__main__':
    main()

