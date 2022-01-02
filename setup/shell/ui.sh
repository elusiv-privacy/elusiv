#!/bin/sh

if [ ! -d "../ui" ]; then
    value=$(<dist/program/pubkeys.json) &&
    cd ../ &&
    git clone git@github.com:SolanaMixing/ui.git &&
    cd ui/ &&
    echo $value > "src/assets/pubkeys.json"
else
    value=$(<dist/program/pubkeys.json) &&
    cd ../ui &&
    cp "../pubkeys.json" "program_keys.json" &&
    echo $value > "src/assets/pubkeys.json"
fi