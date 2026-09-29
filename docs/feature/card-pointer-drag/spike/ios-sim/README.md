# iOS Simulator probe harness

Drives MobileSafari through the spike probes with system-synthesised touches, and
collects each probe's log through `../serve.py` (`?beacon=1`).

    cd .. && python3 serve.py 8000 &            # probe server + /log collector -> probe-log.txt
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
    xcrun simctl boot "iPhone 16 Pro Max"       # reboot between runs: a lingering Safari
                                                # fails with "has not loaded accessibility"
    xcodebuild test -project IOSProbe.xcodeproj -scheme Probe \
      -destination 'platform=iOS Simulator,name=iPhone 16 Pro Max' \
      -only-testing:ProbeUITests/ProbeUITests/testProbeV6
    python3 summ.py ../probe-log.txt v6          # one line per gesture

Synthesised touches start moving at once, so the simulator cannot reproduce a
resting thumb or iOS's drag-interaction stall. Real devices remain the oracle (D19).
