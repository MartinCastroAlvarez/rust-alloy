#!/bin/bash

# This script handles Anvil state persistence
# On first run, Anvil starts fresh and dumps state to a JSON file
# On subsequent runs, it loads the previous state, maintaining accounts and deployed contracts
# This allows for persistent blockchain state between container restarts
# Without this script, Anvil would fail to start when the state file doesn't exist

# Check if state file exists
if [ -f "/data/anvil_state.json" ]; then
    # Start anvil with state loading
    anvil --host "0.0.0.0" --dump-state "/data/anvil_state.json" --load-state "/data/anvil_state.json" --gas-limit 3000000000
else
    # Start anvil without loading state, but still dump state
    anvil --host "0.0.0.0" --dump-state "/data/anvil_state.json" --gas-limit 3000000000

fi 