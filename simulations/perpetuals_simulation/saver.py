
import os
import shutil

def main():
    for file in os.listdir('runs'):
        if file.endswith('.xlsx'):
            source_file = f'runs/{file}'
            destination_file = f'runs_cp/{file}'

            try:
                shutil.copy(source_file, destination_file)
                print(f"File '{source_file}' copied to '{destination_file}' successfully.")
            except FileNotFoundError:
                print("Error: Source file not found.")
            except PermissionError:
                print("Error: Permission denied. Check if you have the necessary permissions.")
            except Exception as e:
                print(f"An error occurred: {str(e)}")
        

if __name__ == "__main__":
    main()