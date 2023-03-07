#!/bin/bash

# Usage: circuit_name out_dir?

DEFAULT_DIR=elusiv/src/proof/vkeys

CIRCUIT_NAME=$1

if [ -z "$2" ]; then
  DIR=$DEFAULT_DIR
else
  DIR=$2
fi

rm -rf temp

git clone --depth=1 --branch=master --single-branch https://github.com/elusiv-privacy/circuits.git ./temp/circuits &&
mkdir -p $DIR/$CIRCUIT_NAME &&
cp ./temp/circuits/bin/$CIRCUIT_NAME/$CIRCUIT_NAME.vkey $DIR/$CIRCUIT_NAME/elusiv_vkey.bin &&
cp ./temp/circuits/bin/$CIRCUIT_NAME/verification_key.json $DIR/$CIRCUIT_NAME/verification_key.json

rm -rf temp