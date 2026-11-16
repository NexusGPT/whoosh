#!/bin/sh
# round: no-rig
# Manifest cell swift-unit: the Swift Testing functions of Tests/Unit, through `swift test` (see _swift.sh).
. "$(dirname "$0")/_sum.sh"
. "$(dirname "$0")/_swift.sh"
swift_cell Unit; rc=$?
emit "$SW_N" "$SW_F" 0; exit $rc
