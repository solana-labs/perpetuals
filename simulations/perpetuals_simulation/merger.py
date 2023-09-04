import pandas as pd
import os
import openpyxl
from parts.utilities.utils import * 

def main():
    starting_event = 1
    ending_event = 9
    number_of_mc = 10
    for i in range(starting_event, ending_event):
        dfs = []
        for j in range(1, number_of_mc + 1):

            df = pd.read_excel(os.path.join('runs', f'event_{i}_mc{j}.xlsx'), sheet_name='Sheet', engine='openpyxl')
            btc_time = df['btc_time'].shift(-1)
            eth_time = df['eth_time'].shift(-1)
            sol_time = df['sol_time'].shift(-1)
            for col in df.columns:
                df[col] = pd.to_numeric(df[col], errors='coerce')

            dfs.append(df)

        combined_df = pd.concat(dfs, ignore_index=True)
        mean_df = combined_df.groupby(combined_df.columns[0]).mean()
        mean_df['btc_time'] = btc_time
        mean_df['eth_time'] = eth_time
        mean_df['sol_time'] = sol_time

        to_xslx(mean_df, os.path.join('runs_merged', f'merged_event_{i}')) 
        print('event', i, 'is complete')

if __name__ == "__main__":
    main()
